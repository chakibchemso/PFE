//! Data pipeline task: read vitals → fuse to u32 → encrypt → send via data channel.

use alloc::vec::Vec;
use defmt::info;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Sender as ChannelSender;
use embassy_sync::watch::Receiver;

use crate::crypto;

/// Reads vitals, fuses into u32 LE, encrypts, sends via data channel.
#[embassy_executor::task]
pub async fn pipeline_task(
    cipher: crypto::Ascon,
    mut vitals_rx: Receiver<'static, CriticalSectionRawMutex, (u8, u8, u8), 2>,
    data_tx: ChannelSender<'static, CriticalSectionRawMutex, Vec<u8>, 5>,
) {
    loop {
        let (bpm, spo2, temp) = vitals_rx.changed().await;

        info!("Vitals: BPM={} SpO2={} Temp={}", bpm, spo2, temp);

        let packed: u32 = (bpm as u32) | ((spo2 as u32) << 8) | ((temp as u32) << 16);
        let data = packed.to_le_bytes();

        let (ciphertext, nonce) = cipher.encrypt(&data);
        info!(
            "Encrypted: {}",
            crate::utils::print_hex(&ciphertext).as_str()
        );

        let mut payload = Vec::new();
        payload.extend_from_slice(nonce.as_slice());
        payload.extend_from_slice(ciphertext.as_slice());
        data_tx.send(payload).await;
    }
}
