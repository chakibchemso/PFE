use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Sender;
use embassy_time::{Duration, Ticker};

use crate::drivers::die_temp::DieTempDriver;

/// Reads the ESP32 die temperature every 2 seconds and publishes to the bus.
#[embassy_executor::task]
pub async fn die_temp_task(
    sensor: DieTempDriver,
    sender: Sender<'static, CriticalSectionRawMutex, i8, 2>,
) {
    let mut ticker = Ticker::every(Duration::from_secs(2));

    loop {
        let temp = sensor.read_celsius() as i8;
        sender.send(temp);
        ticker.next().await;
    }
}
