use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Sender;
use esp_hal::rng::Rng;

use crate::app::bus::SystemBus;
use crate::crypto;

pub mod driver;
pub mod pipeline;
pub mod task;
pub mod ui;

use pipeline::pipeline_task;
use task::fake_vitals_task;

/// Spawn the sensing service: vitals producer + encryption pipeline.
///
/// Currently spawns fake_vitals_task until MAX30102 is soldered.
pub fn register(spawner: &Spawner, rng: Rng, cipher: crypto::Ascon, bus: &'static SystemBus) {
    let vitals_sender: Sender<'static, CriticalSectionRawMutex, (u8, u8, u8), 2> =
        bus.vitals.sender();

    // Vitals producer (fake for now)
    spawner.spawn(fake_vitals_task(rng, vitals_sender)).unwrap();

    // Encryption pipeline: vitals → encrypt → data_channel
    let vitals_rx = bus.vitals.receiver().expect("vitals receiver for pipeline");
    let data_tx = bus.data_channel.sender();
    spawner
        .spawn(pipeline_task(cipher, vitals_rx, data_tx))
        .unwrap();
}
