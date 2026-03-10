#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::{
    clock::CpuClock,
    i2c::master::{Config, I2c},
    timer::timg::TimerGroup,
};
use esp_radio::Controller;
use esp32::{DATA_CHANNEL, alloc::vec::Vec, crypto, mk_static, mqtt, oxymeter, utils, wifi};
use static_cell::StaticCell;

esp_bootloader_esp_idf::esp_app_desc!();
getrandom::register_custom_getrandom!(utils::custom_getrandom);

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    rtt_target::rtt_init_defmt!();

    //? Initialize peripherals and clocks
    let p = {
        let conf = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
        esp_hal::init(conf)
    };

    //? Initialize heap
    let _ = {
        esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744);
        esp_alloc::heap_allocator!(size: 64 * 1024);
    };

    //? Initialize RTOS (required for embassy and radio)
    let _ = {
        let timg0 = TimerGroup::new(p.TIMG0);
        esp_rtos::start(timg0.timer0);
    };

    //? Initialize radio controller
    let rd_ctrl = mk_static!(
        Controller<'static>,
        esp_radio::init().expect("Failed to initialize radio controller")
    );

    //? Initialize WiFi
    let (wf_ctrl, wf_device) =
        esp_radio::wifi::new(rd_ctrl, p.WIFI, esp_radio::wifi::Config::default())
            .expect("Failed to initialize WiFi control");

    //? Initialize network stack
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

    //? Spawn WiFi and network tasks
    let _ = {
        spawner.spawn(wifi::connection_task(wf_ctrl)).unwrap();
        spawner.spawn(wifi::net_task(runner)).unwrap();

        info!("Waiting for IP address...");
        stack.wait_config_up().await;

        let ip_info = stack.config_v4().unwrap();
        info!("Connected! Got IP: {}", ip_info.address);
    };

    info!("Embassy initialized!");

    let mut oxymeter = {
        let i2c = I2c::new(p.I2C0, Config::default())
            .expect("Failed to initialize I2C")
            .with_sda(p.GPIO1)
            .with_scl(p.GPIO2);
        oxymeter::OxymeterHandle::start(&spawner, i2c)
            .await
            .expect("Failed to initialize oxymeter")
    };

    let cipher = {
        let key = b"very secret key!";
        crypto::Ascon::new(key)
    };

    // TODO: Spawn some tasks
    let _ = spawner;
    spawner.spawn(mqtt::mqtt_task(stack)).unwrap();

    loop {
        info!("loop start!");

        // !acquisition
        // let bpm = oxymeter.read_bpm();
        // let spo2 = oxymeter.read_spo2();
        // let temp = oxymeter.read_temp();
        let bpm = oxymeter.bpm();
        let spo2 = oxymeter.spo2();
        let temp = oxymeter.temp();
        info!("Sensor data: BPM: {}, SPO2: {}, Temp: {}", bpm, spo2, temp);

        // !fusion
        let data = {
            let mut out = [0u8; 12];
            out[0..4].copy_from_slice(&bpm.to_le_bytes());
            out[4..8].copy_from_slice(&spo2.to_le_bytes());
            out[8..12].copy_from_slice(&temp.to_le_bytes());
            out
        };
        info!("Prcrypted data: {}", utils::print_hex(&data).as_str());

        // !encryption
        let (ciphertext, nonce) = cipher.encrypt(&data);
        info!("Encrypted data: {}", utils::print_hex(&ciphertext).as_str());

        // !transport
        let _ = {
            let mut payload = Vec::new();
            payload.extend_from_slice(nonce.as_slice());
            payload.extend_from_slice(ciphertext.as_slice());
            DATA_CHANNEL.send(payload).await;
        };

        // !decryption
        let plaintext = cipher.decrypt(&ciphertext, &nonce);
        info!("Decrypted data: {}", utils::print_hex(&plaintext).as_str());
        // info!("Decrypted data: {:X}", plaintext.as_slice());

        // !parsing
        let (bpm, spo2, temp) = (
            f32::from_le_bytes(plaintext[0..4].try_into().unwrap()),
            f32::from_le_bytes(plaintext[4..8].try_into().unwrap()),
            f32::from_le_bytes(plaintext[8..12].try_into().unwrap()),
        );
        info!("Decrypted: BPM: {}, SPO2: {}, Temp: {}", bpm, spo2, temp);

        assert_eq!(&plaintext, &data);

        info!(
            "heap: used {} bytes, free {} bytes",
            esp_alloc::HEAP.used(),
            esp_alloc::HEAP.free()
        );

        Timer::after(Duration::from_millis(1000)).await;
    }
}
