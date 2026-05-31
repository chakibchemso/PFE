#![no_std]
#![no_main]

use defmt::{info, trace};
use embassy_executor::Spawner;
use esp_hal::{
    Async,
    dma::{DmaRxBuf, DmaTxBuf},
    gpio::Output,
    interrupt::software::SoftwareInterruptControl,
    spi::master::SpiDma,
    system::Stack,
    timer::timg::TimerGroup,
};
use esp_rtos::{embassy::Executor, start_second_core};
use esp32::{
    app::bus::SystemBus,
    config, crypto,
    drivers::bus::{I2cBus, I2cPeripheral},
    mk_static,
    services::{self, storage::StorageService},
    system::{self, board::init_board, init_system},
    ui, utils,
};
use static_cell::StaticCell;
use utils::SendWrap;

esp_bootloader_esp_idf::esp_app_desc!();
getrandom::register_custom_getrandom!(utils::custom_getrandom);

// ── Core 1 statics ────────────────────────────────────────────────────────

/// Stack for core 1's scheduler + embassy executor + Slint rendering pipeline.
static CORE1_STACK: StaticCell<Stack<65536>> = StaticCell::new();

/// Thread-mode executor for core 1 (LVGL render loop + touch).
static CORE1_EXECUTOR: StaticCell<Executor> = StaticCell::new();

// ── Core 1 bootstrap ──────────────────────────────────────────────────────

/// One-shot init for core 1: display, LVGL flush pipeline, touch + rendering.
///
/// Runs once at startup, spawns the persistent flush, render, and touch tasks.
#[embassy_executor::task]
async fn core1_bootstrap(
    spawner: Spawner,
    hi_spawner: embassy_executor::SendSpawner,
    qspi_spi: SendWrap<SpiDma<'static, Async>>,
    qspi_rx: SendWrap<DmaRxBuf>,
    qspi_tx: SendWrap<DmaTxBuf>,
    cs: SendWrap<Output<'static>>,
    rst: SendWrap<Output<'static>>,
    touch_rst: SendWrap<Output<'static>>,
    touch_i2c: SendWrap<I2cPeripheral>,
    render_config: ui::RenderConfig,
) {
    trace!("Core 1: initialising display…");

    let display =
        services::rendering::init_display(qspi_spi.0, qspi_rx.0, qspi_tx.0, cs.0, rst.0).await;
    let oxivgl_display = services::rendering::OxivglDisplay(display);

    // 1. Spawn flush task on high-priority interrupt executor.
    services::rendering::register_flush(&hi_spawner, oxivgl_display);

    // 2. Allocate LVGL double-buffers (DMA-aligned, 'static lifetime).
    let bufs = services::rendering::task::take_lvgl_buffers();

    // 3. Spawn LVGL render task on thread-mode executor.
    spawner.spawn(services::rendering::task::render_task(bufs).unwrap());

    // 4. Spawn touch task on thread-mode executor.
    spawner.spawn(
        services::touch::task::touch_task(touch_i2c.0, touch_rst.0, render_config)
            .unwrap(),
    );

    info!("Core 1: LVGL + flush + touch running");

    // Keep the executor alive
    loop {
        embassy_time::Timer::after_secs(60).await;
    }
}

// ── Main (core 0) ─────────────────────────────────────────────────────────

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // Initialize system (clocks, heap)
    let p = init_system();

    // Create the System Bus — central IPC manifest for all services
    let bus: &'static SystemBus = mk_static!(SystemBus, SystemBus::new());

    // Initialize board peripherals — production QSPI + I2C pinout
    let board_periph = init_board(
        p.PSRAM, p.SPI2, p.DMA_CH0, p.I2C0, // peripherals
        p.GPIO4, p.GPIO5, p.GPIO6, p.GPIO7,  // QSPI SIO0–SIO3
        p.GPIO11, // TP_INT
        p.GPIO12, // LCD_CS
        p.GPIO13, // LCD_TE
        p.GPIO14, p.GPIO15, // I2C SCL / SDA
        p.GPIO38, // QSPI_SCLK
        p.GPIO39, // LCD_RST
        p.GPIO40, // TP_RST
    );

    // Initialize RTOS timer + extract software interrupts
    let sw_int1;
    let sw_int2;
    {
        let sw_interrupt = SoftwareInterruptControl::new(p.SW_INTERRUPT);
        sw_int1 = sw_interrupt.software_interrupt1;
        sw_int2 = sw_interrupt.software_interrupt2;
        let timg0 = TimerGroup::new(p.TIMG0);
        esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);
    }

    // Initialize WiFi radio + network stack
    let net = system::net::init(p.WIFI).await;

    // ── I²C bus manager ────────────────────────────────────────────────
    let i2c_bus = mk_static!(
        I2cBus,
        I2cBus::new(board_periph.i2c_bus, esp32::system::board::I2C_FREQ_KHZ)
    );

    // Create one device handle per peripheral
    let touch_i2c = i2c_bus.device(0x5A, "touch");
    let oxymeter_i2c = i2c_bus.device(0x57, "oxymeter");

    // Spawn WiFi service (connection + network stack tasks)
    services::wifi::register(&spawner, net.controller, net.runner, bus);
    info!("WiFi connecting in background…");

    // ── Start core 1: LVGL rendering + flush + touch ───────────────────
    {
        let core1_stack = CORE1_STACK.init(Stack::new());
        let render_config = ui::RenderConfig::production();

        // Wrap !Send peripherals for cross-core transfer.
        let qspi_spi = SendWrap(board_periph.qspi_spi);
        let qspi_rx = SendWrap(board_periph.qspi_rx);
        let qspi_tx = SendWrap(board_periph.qspi_tx);
        let cs = SendWrap(board_periph.display_cs);
        let rst = SendWrap(board_periph.display_rst);
        let t_rst = SendWrap(board_periph.touch_rst);
        let t_i2c = SendWrap(touch_i2c);

        start_second_core(p.CPU_CTRL, sw_int1, core1_stack, move || {
            let executor = CORE1_EXECUTOR.init(Executor::new());
            executor.run(|core1_spawner| {
                // Create interrupt executor for the flush task.
                let int_exec = mk_static!(
                    esp_rtos::embassy::InterruptExecutor::<2>,
                    esp_rtos::embassy::InterruptExecutor::new(sw_int2)
                );
                let hi_spawner = int_exec.start(esp_hal::interrupt::Priority::min());

                core1_spawner.spawn(
                    core1_bootstrap(
                        core1_spawner,
                        hi_spawner,
                        qspi_spi,
                        qspi_rx,
                        qspi_tx,
                        cs,
                        rst,
                        t_rst,
                        t_i2c,
                        render_config,
                    )
                    .unwrap(),
                );
            });
        });

        info!("Core 1: started");
    }

    let cipher = crypto::Ascon::new(config::ASCON_KEY);

    // One-time crypto self-test
    {
        let test_data = [0xAAu8; 4];
        let (ct, nonce) = cipher.encrypt(&test_data);
        let pt = cipher.decrypt(&ct, &nonce);
        assert_eq!(&pt[..], &test_data, "crypto self-test failed!");
        info!("Crypto self-test passed");
    }

    // Spawn MQTT service (consumes encrypted payloads from bus.data_channel)
    services::mqtt::register(&spawner, net.stack, bus);

    // Spawn sensing service (MAX30102 vitals producer + die temp + encryption pipeline)
    services::sensing::register(&spawner, oxymeter_i2c, cipher, bus, unsafe {
        core::mem::transmute(p.SENS)
    })
    .await;

    // GPS service disabled — antenna/RF issues pending hardware fix
    // let gps_i2c = i2c_bus.device(0x50, "gps");
    // services::gps::register(&spawner, gps_i2c, bus);

    // Storage smoke test
    {
        let storage = mk_static!(StorageService, StorageService::new());
        storage.write("smoke_test", b"hello").await;
        let val = storage.read("smoke_test").await;
        info!("Storage smoke test: {:?}", val.map(|v| v.len()));
    }

    // Idle loop
    info!(
        "System running. Free heap: {} bytes",
        esp_alloc::HEAP.free()
    );
    loop {
        embassy_time::Timer::after_secs(60).await;
        info!(
            "heap: used {} bytes, free {} bytes",
            esp_alloc::HEAP.used(),
            esp_alloc::HEAP.free()
        );
    }
}
