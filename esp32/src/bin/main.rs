#![no_std]
#![no_main]

use defmt::{info, trace};
use embassy_embedded_hal::shared_bus::asynch::spi::SpiDeviceWithConfig;
use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, mutex::Mutex};
use esp_hal::time::Rate;
use esp_hal::{
    Async,
    dma::{DmaRxBuf, DmaTxBuf},
    gpio::{Input, Output},
    interrupt::software::SoftwareInterruptControl,
    spi::master::{Config as SpiConfig, Spi, SpiDma},
    system::Stack,
    timer::timg::TimerGroup,
};
use esp_rtos::{embassy::Executor, start_second_core};
use esp32::{
    app::bus::SystemBus,
    crypto,
    drivers::bus::{I2cBus, I2cPeripheral},
    mk_static,
    services::{
        self,
        sensing::ecg,
        storage::{StorageService, StoredConfig},
    },
    system::{self, board::init_board, init_system},
    ui, utils,
};
use static_cell::StaticCell;
use utils::SendWrap;

esp_bootloader_esp_idf::esp_app_desc!();
getrandom::register_custom_getrandom!(utils::custom_getrandom);

// ── Core 1 statics ────────────────────────────────────────────────────────

/// Stack size for core 1's scheduler + embassy executor (12 KB).
const CORE1_STACK_SIZE: usize = 12 * 1024;
static CORE1_STACK: StaticCell<Stack<CORE1_STACK_SIZE>> = StaticCell::new();

/// Thread-mode executor for core 1 (LVGL render loop + touch).
static CORE1_EXECUTOR: StaticCell<Executor> = StaticCell::new();

// ── ECG type aliases ───────────────────────────────────────────────────────

/// Concrete SPI bus type for the MAX30003 ECG sensor (interrupt-based async).
type EcgSpiBus = Spi<'static, Async>;

/// Concrete SPI device type wrapping the bus + CS pin with config.
type EcgSpiDevice =
    SpiDeviceWithConfig<'static, CriticalSectionRawMutex, EcgSpiBus, Output<'static>>;

/// Import types we need from ecg for the concrete task.
use esp32::drivers::ecg::EcgRunner;

// ── ECG concrete task wrapper ─────────────────────────────────────────────

/// Concrete ECG acquisition task.
///
/// Embassy 0.10 does **not** support generic `#[embassy_executor::task]`
/// functions, so we define a concrete wrapper that delegates to the generic
/// [`ecg_acquisition_task`](ecg::ecg_acquisition_task).
#[embassy_executor::task]
async fn ecg_acquisition_task_wrapper(
    runner: EcgRunner<EcgSpiDevice>,
    sender: embassy_sync::watch::Sender<'static, CriticalSectionRawMutex, (u8,), 2>,
) {
    ecg::ecg_acquisition_task(runner, sender).await;
}

// ── Core 1 bootstrap ──────────────────────────────────────────────────────

#[embassy_executor::task]
async fn core1_bootstrap(
    spawner: Spawner,
    hi_spawner: embassy_executor::SendSpawner,
    qspi_spi: SendWrap<SpiDma<'static, Async>>,
    qspi_rx: SendWrap<DmaRxBuf>,
    qspi_tx: SendWrap<DmaTxBuf>,
    cs: SendWrap<Output<'static>>,
    rst: SendWrap<Output<'static>>,
    display_te: SendWrap<Input<'static>>,
    touch_int: SendWrap<Input<'static>>,
    touch_rst: SendWrap<Output<'static>>,
    touch_i2c: SendWrap<I2cPeripheral>,
    render_config: ui::RenderConfig,
    bus: &'static SystemBus,
    storage: &'static StorageService,
    stored_config: &'static Mutex<CriticalSectionRawMutex, StoredConfig>,
) {
    trace!("Core 1: initialising display…");

    let display =
        services::rendering::init_display(qspi_spi.0, qspi_rx.0, qspi_tx.0, cs.0, rst.0).await;
    let send_display = services::rendering::SendDisplay(display);

    let vitals_rx = bus.vitals.receiver();
    let ecg_hr_rx = bus.ecg_hr.receiver();
    let wifi_rx = bus.wifi_status.receiver();
    let mqtt_rx = bus.mqtt_status.receiver();
    let utc_rx = bus.utc_epoch.receiver();
    let battery_rx = bus.battery.receiver();

    spawner.spawn(
        services::ui::task::render_task(
            spawner,
            hi_spawner,
            send_display,
            display_te.0,
            vitals_rx,
            ecg_hr_rx,
            wifi_rx,
            mqtt_rx,
            utc_rx,
            battery_rx,
            storage,
            stored_config,
        )
        .unwrap(),
    );

    spawner.spawn(
        services::touch::task::touch_task(touch_i2c.0, touch_rst.0, render_config, touch_int.0)
            .unwrap(),
    );

    info!("Core 1: LVGL + flush + touch running");

    loop {
        embassy_time::Timer::after_secs(60).await;
    }
}

// ── Main (core 0) ─────────────────────────────────────────────────────────

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    let p = init_system();
    let flash = p.FLASH;

    let bus: &'static SystemBus = mk_static!(SystemBus, SystemBus::new());

    let board_periph = init_board(
        p.PSRAM, p.SPI2, p.DMA_CH0, p.I2C0, p.GPIO4, p.GPIO5, p.GPIO6, p.GPIO7, p.GPIO11, p.GPIO12,
        p.GPIO13, p.GPIO14, p.GPIO15, p.GPIO38, p.GPIO39, p.GPIO40, p.SPI3, p.GPIO16, p.GPIO17,
        p.GPIO43, p.GPIO44,
    );

    let sw_int1;
    let sw_int2;
    {
        let sw_interrupt = SoftwareInterruptControl::new(p.SW_INTERRUPT);
        sw_int1 = sw_interrupt.software_interrupt1;
        sw_int2 = sw_interrupt.software_interrupt2;
        let timg0 = TimerGroup::new(p.TIMG0);
        esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);
    }

    let net = system::net::init(p.WIFI).await;

    let i2c_bus = mk_static!(
        I2cBus,
        I2cBus::new(board_periph.i2c_bus, esp32::system::board::I2C_FREQ_KHZ)
    );

    let touch_i2c = i2c_bus.device(0x5A, "touch");
    let oxymeter_i2c = i2c_bus.device(0x57, "oxymeter");
    let axp2101_i2c = i2c_bus.device(0x34, "axp2101");

    // ── Storage ─────────────────────────────────────────────────────────
    let storage = mk_static!(StorageService, StorageService::new(flash));
    let stored_config = mk_static!(
        Mutex<CriticalSectionRawMutex, StoredConfig>,
        Mutex::new(StoredConfig::load(storage).await)
    );
    let stored_config: &'static Mutex<CriticalSectionRawMutex, StoredConfig> = stored_config;

    // ── Crypto with stored key ──────────────────────────────────────────
    let ascon_key = {
        let cfg = stored_config.lock().await;
        cfg.ascon_key
    };
    let cipher = crypto::Ascon::new(&ascon_key);

    // ── WiFi with stored credentials ────────────────────────────────────
    services::wifi::register(&spawner, net.controller, net.runner, bus, stored_config);
    info!("WiFi connecting in background…");

    services::time::register(&spawner, net.stack, bus);

    // ── Start core 1 ────────────────────────────────────────────────────
    {
        let core1_stack = CORE1_STACK.init(Stack::new());
        let render_config = ui::RenderConfig::production();

        let qspi_spi = SendWrap(board_periph.qspi_spi);
        let qspi_rx = SendWrap(board_periph.qspi_rx);
        let qspi_tx = SendWrap(board_periph.qspi_tx);
        let cs = SendWrap(board_periph.display_cs);
        let rst = SendWrap(board_periph.display_rst);
        let t_rst = SendWrap(board_periph.touch_rst);
        let t_i2c = SendWrap(touch_i2c);
        let display_te = SendWrap(board_periph.display_te);
        let touch_int = SendWrap(board_periph.touch_int);

        start_second_core(p.CPU_CTRL, sw_int1, core1_stack, move || {
            let executor = CORE1_EXECUTOR.init(Executor::new());
            executor.run(|core1_spawner| {
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
                        display_te,
                        touch_int,
                        t_rst,
                        t_i2c,
                        render_config,
                        bus,
                        storage,
                        stored_config,
                    )
                    .unwrap(),
                );
            });
        });

        info!("Core 1: started");
    }

    // Crypto self-test
    {
        let test_data = [0xAAu8; 4];
        let (ct, nonce) = cipher.encrypt(&test_data);
        let pt = cipher.decrypt(&ct, &nonce);
        assert_eq!(&pt[..], &test_data, "crypto self-test failed!");
        info!("Crypto self-test passed");
    }

    // ── ECG FCLK: 32.768 kHz on GPIO18 via LEDC ────────────────────────
    {
        use esp_hal::ledc::channel::{ChannelIFace, Number as ChannelNumber};
        use esp_hal::ledc::timer::{LSClockSource, TimerIFace};
        use esp_hal::ledc::{Ledc, LowSpeed, channel, timer};

        let ledc = Ledc::new(p.LEDC);
        let mut ls_timer = ledc.timer::<LowSpeed>(timer::Number::Timer0);
        TimerIFace::configure(
            &mut ls_timer,
            timer::config::Config {
                clock_source: LSClockSource::APBClk,
                duty: timer::config::Duty::Duty10Bit,
                frequency: Rate::from_hz(32768),
            },
        )
        .unwrap();
        {
            let mut fclk_channel = ledc.channel(ChannelNumber::Channel0, p.GPIO18);
            ChannelIFace::configure(
                &mut fclk_channel,
                channel::config::Config {
                    timer: &ls_timer,
                    duty_pct: 50,
                    drive_mode: esp_hal::gpio::DriveMode::PushPull,
                },
            )
            .unwrap();
            core::mem::forget(fclk_channel);
        }
        core::mem::forget(ls_timer);
        core::mem::forget(ledc);
    }

    // ── ECG SPI device (MAX30003, SPI3) ────────────────────────────────
    static ECG_SPI_BUS: StaticCell<Mutex<CriticalSectionRawMutex, Spi<'static, Async>>> =
        StaticCell::new();
    let ecg_spi_bus = ECG_SPI_BUS.init(Mutex::new(board_periph.ecg_spi));

    let spi_config = SpiConfig::default()
        .with_frequency(Rate::from_mhz(5))
        .with_mode(esp_hal::spi::Mode::_0);
    let ecg_spi_device = SpiDeviceWithConfig::new(ecg_spi_bus, board_periph.ecg_cs, spi_config);

    // ── ECG sensor init as runner ──────────────────────────────────────
    {
        use esp32::drivers::low::max30003::Max30003;

        let sensor = Max30003::new(ecg_spi_device);
        let ecg_runner = ecg::init_sensor(sensor).await;

        let ecg_hr_sender = bus.ecg_hr.sender();
        spawner.spawn(ecg_acquisition_task_wrapper(ecg_runner, ecg_hr_sender).unwrap());

        info!("ECG sensor started");
    }

    services::mqtt::register(&spawner, net.stack, bus);

    services::sensing::register(&spawner, oxymeter_i2c, cipher, bus, unsafe {
        core::mem::transmute(p.SENS)
    })
    .await;

    services::power::register(&spawner, axp2101_i2c, bus).await;

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
