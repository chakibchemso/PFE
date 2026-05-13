#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_net::Config;
use embassy_time::Timer;
use esp_hal::rng::Rng;
use esp_radio::Controller;
use esp32::{
    app::bus::SystemBus,
    config, crypto,
    drivers::bus::{SharedI2cBus, SharedI2cDevice},
    mk_static,
    services::{self, storage::StorageService},
    system::{board::init_board, init_system},
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
    let mut board_periph = init_board(
        p.SPI2, p.DMA_CH0, p.I2C0, p.GPIO4, p.GPIO5, p.GPIO6, p.GPIO7,  // QSPI SIO0–SIO3
        p.GPIO11, // TP_INT
        p.GPIO12, // LCD_CS
        p.GPIO13, // LCD_TE
        p.GPIO14, p.GPIO15, // I2C SCL / SDA
        p.GPIO38, // QSPI_SCLK
        p.GPIO39, // LCD_RST
        p.GPIO40, // TP_RST
    );

    // De‑assert touch reset (CST9217 driver handles full reset sequence in init())
    board_periph.touch_rst.set_high();

    // Wrap I2C bus in a shared mutex for multiple devices
    let shared_i2c_bus = mk_static!(SharedI2cBus, SharedI2cBus::new(board_periph.i2c_bus));

    // Initialize RTOS timer
    {
        use esp_hal::timer::timg::TimerGroup;
        let timg0 = TimerGroup::new(p.TIMG0);
        esp_rtos::start(timg0.timer0);
    }

    // Initialize radio controller
    let rd_ctrl = mk_static!(
        Controller<'static>,
        esp_radio::init().expect("Failed to initialize radio controller")
    );

    // Initialize WiFi
    let (wf_ctrl, wf_device) =
        esp_radio::wifi::new(rd_ctrl, p.WIFI, esp_radio::wifi::Config::default())
            .expect("Failed to initialize WiFi control");

    // Initialize network stack
    let (stack, runner) = {
        let sta_conf = Config::dhcpv4(Default::default());

        let seed = {
            let rng = Rng::new();
            ((rng.random() as u64) << 32) | (rng.random() as u64)
        };

        embassy_net::new(
            wf_device.sta,
            sta_conf,
            mk_static!(
                embassy_net::StackResources<3>,
                embassy_net::StackResources::<3>::new()
            ),
            seed,
        )
    };

    Timer::after_millis(100).await; // fucking pwr mgmt

    // Spawn WiFi service (connection + network stack tasks)
    services::wifi::register(&spawner, wf_ctrl, runner, bus);

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
        shared_i2c_bus,
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
    services::mqtt::register(&spawner, stack, bus);

    // Spawn sensing service (MAX30102 vitals producer + encryption pipeline)
    let oxymeter_i2c = SharedI2cDevice::new(shared_i2c_bus);
    services::sensing::register(&spawner, oxymeter_i2c, cipher, bus).await;

    // GPS service disabled — antenna/RF issues pending hardware fix
    // services::gps::register(&spawner, shared_i2c_bus, bus);

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
