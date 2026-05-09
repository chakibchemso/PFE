use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Sender;
use embassy_time::{Duration, Ticker};
use esp_hal::rng::Rng;

/// Generates plausible-looking vitals at 1 Hz.
#[embassy_executor::task]
pub async fn fake_vitals_task(
    rng: Rng,
    sender: Sender<'static, CriticalSectionRawMutex, (u8, u8, u8), 2>,
) {
    let mut ticker = Ticker::every(Duration::from_millis(1000));

    loop {
        let bpm = 60 + (rng.random() as u32 % 21) as u8;
        let spo2 = 95 + (rng.random() as u32 % 6) as u8;
        let temp = 36 + (rng.random() as u32 % 2) as u8;

        sender.send((bpm, spo2, temp));
        ticker.next().await;
    }
}
