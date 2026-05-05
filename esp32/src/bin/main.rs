#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_net::Config;
use esp_hal::rng::Rng;
use esp_radio::Controller;
use esp32::{
    app::{pipeline::pipeline_task, state},
    config, crypto,
    drivers::{
        bus::SharedI2cBus,
        display::init_display,
        sensor::{init_oxymeter, init_touch},
    },
    mk_static,
    system::{board::init_board, init_system},
    tasks::{mqtt, wifi},
    ui, utils,
};
use static_cell::StaticCell;

esp_bootloader_esp_idf::esp_app_desc!();
getrandom::register_custom_getrandom!(utils::custom_getrandom);

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    // Initialize system (clocks, heap)
    let p = init_system();

    // Initialize board peripherals (SPI, I2C, display pins)
    let board_periph = init_board(
        p.SPI2, p.DMA_CH0, p.I2C0, p.GPIO1, p.GPIO2, p.GPIO8, p.GPIO9, p.GPIO10, p.GPIO11, p.GPIO12,
    );

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

    // Spawn WiFi and network tasks (non-blocking: OS continues while these connect in background)
    spawner.spawn(wifi::connection_task(wf_ctrl)).unwrap();
    spawner.spawn(wifi::net_task(runner)).unwrap();

    info!("WiFi connecting in background...");

    // Create display (consumes the SPI bus)
    let display = init_display(
        board_periph.spi_bus,
        board_periph.display_dc,
        board_periph.display_rst,
        board_periph.display_cs,
    )
    .await;

    // Initialize sensors (oxymeter + touch) on shared I2C bus
    let oxymeter = init_oxymeter(&spawner, shared_i2c_bus)
        .await
        .expect("Failed to initialize oxymeter");

    let touch = init_touch(
        shared_i2c_bus,
        ui::RenderConfig::dev_st7796().panel_width,
        ui::RenderConfig::dev_st7796().panel_height,
    )
    .await
    .expect("Failed to initialize touch controller");

    // Initialize Slint platform and window
    let render_config = ui::RenderConfig::dev_st7796();
    let window = ui::init_platform(render_config.viewport_size);

    // Create shared window handle for touch_task to dispatch events directly
    let shared_window = mk_static!(
        ui::SharedWindowHandle,
        ui::SharedWindowHandle::new(core::cell::RefCell::new(None))
    );

    // Spawn dedicated touch task - dispatches directly to Slint window
    spawner
        .spawn(ui::touch_task(touch, render_config, shared_window))
        .unwrap();

    // Spawn UI task - handles rendering, vitals, clock
    spawner
        .spawn(ui::ui_task(
            render_config,
            display,
            shared_window,
            window,
            state::vitals_receiver(),
        ))
        .unwrap();

    let cipher = crypto::Ascon::new(config::ASCON_KEY);

    // One-time crypto self-test
    {
        let test_data = [0xAAu8; 12];
        let (ct, nonce) = cipher.encrypt(&test_data);
        let pt = cipher.decrypt(&ct, &nonce);
        assert_eq!(&pt[..], &test_data, "crypto self-test failed!");
        info!("Crypto self-test passed");
    }

    // start mqtt
    spawner.spawn(mqtt::mqtt_task(stack)).unwrap();

    // Spawn data pipeline task
    spawner.spawn(pipeline_task(oxymeter, cipher)).unwrap();

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
