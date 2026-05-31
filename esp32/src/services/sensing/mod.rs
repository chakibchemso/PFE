use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Sender;

use crate::app::bus::SystemBus;
use crate::crypto;
use crate::drivers::bus::I2cPeripheral;
use crate::drivers::die_temp::DieTempDriver;

pub mod die_temp;
pub mod driver;
pub mod pipeline;
pub mod task;
use driver::OxymeterHandle;
use pipeline::pipeline_task;

/// Spawn the sensing service: vitals producer + die temp + encryption pipeline.
pub async fn register(
    spawner: &Spawner,
    i2c_device: I2cPeripheral,
    cipher: crypto::Ascon,
    bus: &'static SystemBus,
    tsens: esp_hal::peripherals::SENS<'static>,
) {
    let vitals_sender: Sender<'static, CriticalSectionRawMutex, (u8, u8, u8), 2> =
        bus.vitals.sender();

    // Vitals producer (real MAX30102 oxymeter) — starts acquisition_task internally
    OxymeterHandle::start(spawner, i2c_device, vitals_sender)
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
}
