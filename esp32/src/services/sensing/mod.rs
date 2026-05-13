use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Sender;

use crate::app::bus::SystemBus;
use crate::crypto;
use crate::drivers::bus::SharedI2cDevice;

pub mod driver;
pub mod pipeline;
pub mod task;
pub mod ui;

use driver::OxymeterHandle;
use pipeline::pipeline_task;

/// Spawn the sensing service: vitals producer + encryption pipeline.
pub async fn register(
    spawner: &Spawner,
    i2c_device: SharedI2cDevice,
    cipher: crypto::Ascon,
    bus: &'static SystemBus,
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
    spawner
        .spawn(pipeline_task(cipher, vitals_rx, data_tx))
        .unwrap();
}
