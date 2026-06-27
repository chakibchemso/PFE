use embassy_executor::Spawner;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, watch::Sender};
use embassy_time::{Duration, Ticker};
use esp_hal::gpio::Input;
use esp_hal::peripherals::{ADC1, GPIO3};

use crate::{
    app::bus::SystemBus,
    crypto,
    drivers::{bus::I2cPeripheral, die_temp::DieTempDriver, ntc::create_ntc_driver},
};

pub mod die_temp;
pub mod driver;
pub mod pipeline;
pub mod task;
use driver::OxymeterHandle;
use pipeline::pipeline_task;

/// Spawn the sensing service: vitals producer + die temp + NTC skin temp + encryption pipeline.
pub async fn register(
    spawner: &Spawner,
    i2c_device: I2cPeripheral,
    oxymeter_int: Input<'static>,
    cipher: crypto::Ascon,
    bus: &'static SystemBus,
    tsens: esp_hal::peripherals::SENS<'static>,
    adc1: ADC1<'static>,
    gpio3: GPIO3<'static>,
) {
    let vitals_sender: Sender<'static, CriticalSectionRawMutex, (u8, u8, u8), 2> =
        bus.vitals.sender();

    // Vitals producer (real MAX30102 oxymeter) — starts acquisition_task internally
    OxymeterHandle::start(spawner, i2c_device, oxymeter_int, vitals_sender)
        .await
        .unwrap();

    // Encryption pipeline: vitals → encrypt → data_channel
    let vitals_rx = bus.vitals.receiver().expect("vitals receiver for pipeline");
    let data_tx = bus.data_channel.sender();
    spawner.spawn(pipeline_task(cipher, vitals_rx, data_tx).unwrap());

    // ESP32 die temperature sensor
    let cpu_temp_sender = bus.cpu_temp.sender();
    let die_temp_driver = DieTempDriver::new(tsens);
    spawner.spawn(die_temp::die_temp_task(die_temp_driver, cpu_temp_sender).unwrap());

    // NTC skin temperature reader — updates SKIN_TEMP every 2 seconds
    spawner.spawn(ntc_reading_task(adc1, gpio3).unwrap());
}

/// Background task that reads the NTC thermistor every 2 s and stores the
/// result in [`SKIN_TEMP`](crate::drivers::ntc::SKIN_TEMP).
#[embassy_executor::task]
async fn ntc_reading_task(adc1: ADC1<'static>, gpio3: GPIO3<'static>) {
    let mut driver = create_ntc_driver(adc1, gpio3);
    let mut ticker = Ticker::every(Duration::from_secs(2));

    loop {
        let temp = driver.read_celsius();
        // Clamp to valid body-temperature range and store as u8
        let clamped = (libm::roundf(temp) as i16).clamp(30, 45) as u8;
        crate::drivers::ntc::SKIN_TEMP.store(clamped, core::sync::atomic::Ordering::Relaxed);
        ticker.next().await;
    }
}
