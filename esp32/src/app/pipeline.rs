//! Data pipeline task: sensor → serialize → encrypt → transport via DATA_CHANNEL.

use alloc::vec::Vec;
use defmt::info;
use embassy_time::{Duration, Ticker};

use crate::app::state::DATA_CHANNEL;
use crate::crypto;
use crate::drivers::oxymeter::OxymeterHandle;

/// Main data pipeline task: reads sensor data, encrypts, and sends via MQTT channel.
#[embassy_executor::task]
pub async fn pipeline_task(mut oxymeter: OxymeterHandle, cipher: crypto::Ascon) {
    let mut ticker = Ticker::every(Duration::from_millis(1000));
    loop {
        // ! acquisition
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
        info!(
            "Plaintext data: {}",
            crate::utils::print_hex(&data).as_str()
        );

        // ! encryption
        let (ciphertext, nonce) = cipher.encrypt(&data);
        info!(
            "Encrypted data: {}",
            crate::utils::print_hex(&ciphertext).as_str()
        );

        // ! transport
        let mut payload = Vec::new();
        payload.extend_from_slice(nonce.as_slice());
        payload.extend_from_slice(ciphertext.as_slice());
        DATA_CHANNEL.send(payload).await;

        ticker.next().await;
    }
}
