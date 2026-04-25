#![no_std]
#![no_main]

use core::cell::RefCell;

use defmt::info;
use embassy_executor::Spawner;
use embassy_time::{Duration, Ticker};
use esp_hal::{
    clock::CpuClock,
    dma::{DmaRxBuf, DmaTxBuf},
    dma_descriptors,
    gpio::{Level, Output, OutputConfig},
    i2c::master::{Config as I2cConfig, I2c},
    psram::{FlashFreq, PsramConfig, SpiRamFreq, SpiTimingConfigCoreClock},
    spi::{
        Mode,
        master::{Config as SpiConfig, Spi},
    },
    time::Rate,
    timer::timg::TimerGroup,
};
use esp_radio::Controller;
use esp32::{
    DATA_CHANNEL, alloc::vec::Vec, crypto, mk_static, mqtt, oxymeter, touch, ui, utils, wifi,
};
use lcd_async::options::{ColorInversion, ColorOrder, Orientation};
use lcd_async::{Builder, interface::SpiInterface, models::ST7796};
use static_cell::StaticCell;

esp_bootloader_esp_idf::esp_app_desc!();
getrandom::register_custom_getrandom!(utils::custom_getrandom);

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    rtt_target::rtt_init_defmt!();

    // ? Initialize peripherals, clocks, and PSRAM
    let config = esp_hal::Config::default()
        .with_cpu_clock(CpuClock::max())
        .with_psram(PsramConfig {
            flash_frequency: FlashFreq::FlashFreq80m,
            ram_frequency: SpiRamFreq::Freq80m,
            core_clock: Some(SpiTimingConfigCoreClock::SpiTimingConfigCoreClock160m),
            ..Default::default()
        });
    let p = esp_hal::init(config);

    // ? Initialize heap: internal SRAM + PSRAM
    {
        // Internal SRAM heap in regular DRAM.
        esp_alloc::heap_allocator!(size: 64 * 1024);
        // Additional SRAM reclaimed from unused ROM sections.
        esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);
        // PSRAM heap (16MB on ESP32-S3N32R16V)
        esp_alloc::psram_allocator!(p.PSRAM, esp_hal::psram);
    }

    // ? Initialize RTOS (required for embassy and radio)
    {
        let timg0 = TimerGroup::new(p.TIMG0);
        esp_rtos::start(timg0.timer0);
    }

    // ? Initialize radio controller
    let rd_ctrl = mk_static!(
        Controller<'static>,
        esp_radio::init().expect("Failed to initialize radio controller")
    );

    // ? Initialize WiFi
    let (wf_ctrl, wf_device) =
        esp_radio::wifi::new(rd_ctrl, p.WIFI, esp_radio::wifi::Config::default())
            .expect("Failed to initialize WiFi control");

    // ? Initialize network stack
    let (stack, runner) = {
        let sta_conf = embassy_net::Config::dhcpv4(Default::default());

        let seed = {
            let rng = esp_hal::rng::Rng::new();
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

    // ? Spawn WiFi and network tasks
    {
        spawner.spawn(wifi::connection_task(wf_ctrl)).unwrap();
        spawner.spawn(wifi::net_task(runner)).unwrap();

        info!("Waiting for IP address...");
        stack.wait_config_up().await;

        let ip_info = stack.config_v4().unwrap();
        info!("Connected! Got IP: {}", ip_info.address);
    };

    info!("initialized!");

    // 1. Display SPI configuration
    let sclk = p.GPIO12;
    let mosi = p.GPIO11;

    let (rx_descriptors, tx_descriptors) = dma_descriptors!(32000, 32000);
    let dma_rx_buf = DmaRxBuf::new(rx_descriptors, mk_static!([u8; 32000], [0; 32000])).unwrap();
    let dma_tx_buf = DmaTxBuf::new(tx_descriptors, mk_static!([u8; 32000], [0; 32000])).unwrap();

    let spi = Spi::new(
        p.SPI2,
        SpiConfig::default()
            .with_frequency(Rate::from_mhz(80))
            .with_mode(Mode::_0),
    )
    .expect("Failed to initialize SPI")
    .with_sck(sclk)
    .with_mosi(mosi)
    .with_dma(p.DMA_CH0)
    .with_buffers(dma_rx_buf, dma_tx_buf)
    .into_async();

    // 2. Display control pins
    let dc = Output::new(p.GPIO9, Level::Low, OutputConfig::default());
    let rst = Output::new(p.GPIO8, Level::Low, OutputConfig::default());
    let cs = Output::new(p.GPIO10, Level::High, OutputConfig::default());

    // 3. Shared SPI bus for async DMA access (embassy async mutex)
    let spi_bus = mk_static!(ui::DisplaySpiBus, ui::DisplaySpiBus::new(spi));
    // Async SpiDevice wrapper for shared bus access
    let spi_dev = ui::DisplaySpiDevice::new(spi_bus, cs);

    // 4. Initialize the display via lcd-async SpiInterface
    let di = SpiInterface::new(spi_dev, dc);
    let mut delay = embassy_time::Delay;
    let mut display = Builder::new(ST7796, di)
        .reset_pin(rst)
        .color_order(ColorOrder::Bgr)
        .invert_colors(ColorInversion::Inverted)
        .init(&mut delay)
        .await
        .unwrap();

    let render_config = ui::RenderConfig::dev_st7796();
    let mut display_orientation = Orientation::default();
    if render_config.display_mirror_x {
        display_orientation = display_orientation.flip_horizontal();
    }
    if render_config.display_mirror_y {
        display_orientation = display_orientation.flip_vertical();
    }
    display.set_orientation(display_orientation).await.unwrap();

    let i2c_bus = {
        let i2c = I2c::new(
            p.I2C0,
            I2cConfig::default().with_frequency(Rate::from_khz(400)),
        )
        .expect("Failed to initialize I2C")
        .with_sda(p.GPIO1)
        .with_scl(p.GPIO2);
        mk_static!(
            touch::SharedI2cBus,
            touch::SharedI2cBus::new(RefCell::new(i2c))
        )
    };

    let touch_i2c_bus = {
        let i2c = I2c::new(
            p.I2C1,
            I2cConfig::default().with_frequency(Rate::from_khz(400)),
        )
        .expect("Failed to initialize I2C")
        .with_sda(p.GPIO40)
        .with_scl(p.GPIO41);
        mk_static!(
            touch::SharedI2cBus,
            touch::SharedI2cBus::new(RefCell::new(i2c))
        )
    };

    let mut oxymeter = {
        let sensor_i2c = touch::SharedI2cDevice::new(i2c_bus);
        oxymeter::OxymeterHandle::start(&spawner, sensor_i2c)
            .await
            .expect("Failed to initialize oxymeter")
    };

    let touch = touch::TouchDevice::new(
        touch::TouchControllerKind::Ft6336,
        touch::SharedI2cDevice::new(touch_i2c_bus),
        render_config.panel_width,
        render_config.panel_height,
    )
    .expect("Failed to initialize touch controller");

    // Initialize Slint platform and window
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
            oxymeter::vitals_receiver(),
        ))
        .unwrap();

    let cipher = {
        let key = b"very secret key!";
        crypto::Ascon::new(key)
    };

    // start mqtt
    spawner.spawn(mqtt::mqtt_task(stack)).unwrap();

    // ! main loop
    let mut ticker = Ticker::every(Duration::from_millis(1000));
    loop {
        info!("loop start!");

        // ! acquisition
        // let bpm = oxymeter.read_bpm();
        // let spo2 = oxymeter.read_spo2();
        // let temp = oxymeter.read_temp();
        let bpm = oxymeter.bpm();
        let spo2 = oxymeter.spo2();
        let temp = oxymeter.temp();
        info!("Sensor data: BPM: {}, SPO2: {}, Temp: {}", bpm, spo2, temp);

        // ! fusion
        let data = {
            let mut out = [0u8; 12];
            out[0..4].copy_from_slice(&bpm.to_le_bytes());
            out[4..8].copy_from_slice(&spo2.to_le_bytes());
            out[8..12].copy_from_slice(&temp.to_le_bytes());
            out
        };
        info!("Prcrypted data: {}", utils::print_hex(&data).as_str());

        // ! encryption
        let (ciphertext, nonce) = cipher.encrypt(&data);
        info!("Encrypted data: {}", utils::print_hex(&ciphertext).as_str());

        // ! transport
        {
            let mut payload = Vec::new();
            payload.extend_from_slice(nonce.as_slice());
            payload.extend_from_slice(ciphertext.as_slice());
            DATA_CHANNEL.send(payload).await;
        }

        // ! decryption
        let plaintext = cipher.decrypt(&ciphertext, &nonce);
        info!("Decrypted data: {}", utils::print_hex(&plaintext).as_str());

        // ! parsing
        let (bpm, spo2, temp) = (
            f32::from_le_bytes(plaintext[0..4].try_into().unwrap()),
            f32::from_le_bytes(plaintext[4..8].try_into().unwrap()),
            f32::from_le_bytes(plaintext[8..12].try_into().unwrap()),
        );
        info!("Decrypted: BPM: {}, SPO2: {}, Temp: {}", bpm, spo2, temp);

        assert_eq!(&plaintext, &data);

        // utils::fade_disp_colors(&mut display).await;

        info!(
            "heap: used {} bytes, free {} bytes",
            esp_alloc::HEAP.used(),
            esp_alloc::HEAP.free()
        );

        ticker.next().await;
    }
}
