#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use esp_hal::{interrupt::software::SoftwareInterruptControl, timer::timg::TimerGroup};
use esp32::{
    app::bus::SystemBus,
    config, crypto,
    drivers::bus::I2cBus,
    mk_static,
    services::{self, storage::StorageService},
    system::{self, board::init_board, init_system},
    ui, utils,
};
use static_cell::StaticCell;

esp_bootloader_esp_idf::esp_app_desc!();
getrandom::register_custom_getrandom!(utils::custom_getrandom);

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // Initialize system (clocks, heap)
    let p = init_system();

    // Create the System Bus — central IPC manifest for all services
    let bus = mk_static!(SystemBus, SystemBus::new());

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

    // Initialize RTOS timer
    {
        let timg0 = TimerGroup::new(p.TIMG0);
        let sw_interrupt = SoftwareInterruptControl::new(p.SW_INTERRUPT);
        esp_rtos::start(timg0.timer0, sw_interrupt.software_interrupt0);
    }

    // Initialize WiFi radio + network stack
    let net = system::net::init(p.WIFI).await;

    // De‑assert touch reset (CST9217 driver handles full reset sequence in init())
    // board_periph.touch_rst.set_high();

    // ── I²C bus manager ────────────────────────────────────────────────
    let i2c_bus = mk_static!(
        I2cBus,
        I2cBus::new(board_periph.i2c_bus, esp32::system::board::I2C_FREQ_KHZ)
    );

    // Scan the bus — print every device that ACKs
    // {
    //     let scan = i2c_bus.scan().await;
    //     info!("I2C bus scan:");
    //     for addr in 1..=0x7Fu8 {
    //         if scan[addr as usize] {
    //             info!("  0x{:02X} ACK", addr);
    //         }
    //     }
    // }

    // Create one device handle per peripheral — the single source of truth
    let touch_i2c = i2c_bus.device(0x5A, "touch");
    let oxymeter_i2c = i2c_bus.device(0x57, "oxymeter");

    // Spawn WiFi service (connection + network stack tasks)
    services::wifi::register(&spawner, net.controller, net.runner, bus);

    info!("WiFi connecting in background...");

    // Create display (consumes the SPI bus + CS + RST pins)
    let display = services::rendering::display::init_display(
        board_periph.qspi_bus,
        board_periph.display_cs,
        board_periph.display_rst,
    )
    .await;

    // Initialize Slint platform and window
    let render_config = ui::RenderConfig::production();
    let window = services::rendering::platform::init_platform(render_config.viewport_size);

    // Create shared window handle for touch service to dispatch events
    let shared_window = mk_static!(
        services::rendering::SharedWindow,
        services::rendering::SharedWindow::new(core::cell::RefCell::new(None))
    );

    // Spawn touch service (CST9217 driver + INT-based task)
    services::touch::register(
        &spawner,
        touch_i2c,
        shared_window,
        board_periph.touch_int,
        board_periph.touch_rst,
        render_config,
    );

    // Spawn rendering service (display driver + Slint platform + render loop)
    services::rendering::register(&spawner, render_config, display, shared_window, window, bus);

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
