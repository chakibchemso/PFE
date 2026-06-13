//! High-level ECG driver wrapping the MAX30003 for the sensing service.
//!
//! Minimal configuration — no lead-off detection, just raw ECG acquisition
//! with extensive defmt logging for debugging.

use defmt::{info, warn};
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, channel::Channel, watch::Sender};
use embassy_time::{Duration, Instant, Timer};
use embedded_hal_async::delay::DelayNs;
use embedded_hal_async::spi::SpiDevice;

use crate::drivers::low::max30003::Max30003;
use crate::dsp::{BpmCalculator, FIR_COEFFS, FirFilter, MovingMeanSubtractor, Smoother};

// ── Static channels ───────────────────────────────────────────────────────────

/// Intermediary ECG sample buffer for the UI waveform chart.
pub static ECG_SAMPLE_BUFFER: Channel<CriticalSectionRawMutex, i32, 128> = Channel::new();

// ── Runner ────────────────────────────────────────────────────────────────────

/// Minimal ECG acquisition runner — no lead-off detection, just raw data.
pub struct EcgRunner<SPI> {
    sensor: Max30003<SPI>,

    // DSP pipeline
    dc_block: MovingMeanSubtractor<100>,
    bandpass: FirFilter<201>,

    // Heart rate
    bpm_calc: BpmCalculator,
    bpm_smoother: Smoother,
}

impl<SPI, SpiError> EcgRunner<SPI>
where
    SPI: SpiDevice<Error = SpiError>,
    SpiError: core::fmt::Debug,
{
    pub async fn run(mut self, sender: Sender<'static, CriticalSectionRawMutex, (u8,), 2>) -> ! {
        // ── Verify SPI communication ────────────────────────────────────
        match self.sensor.read_device_id().await {
            Ok(true) => info!("ECG: device ID OK (revision 5.x)"),
            Ok(false) => warn!("ECG: device ID mismatch — check SPI wiring"),
            Err(e) => warn!("ECG: device ID read error: {}", defmt::Debug2Format(&e)),
        }

        // ── Read initial status ─────────────────────────────────────────
        match self.sensor.read_status().await {
            Ok(s) => {
                let eint = (s >> 23) & 1;
                let eovf = (s >> 22) & 1;
                let dcloff = (s >> 20) & 1;
                let pllint = (s >> 8) & 1;
                info!(
                    "ECG: initial status 0x{:06X}  EINT={} EOVF={} DCLOFF={} PLLINT={}",
                    s, eint, eovf, dcloff, pllint
                );
            }
            Err(e) => warn!(
                "ECG: initial status read error: {}",
                defmt::Debug2Format(&e)
            ),
        }

        // Reset FIFO to known state
        self.sensor.fifo_reset().await.unwrap();
        info!("ECG: FIFO reset, starting acquisition loop");

        let mut cycles_without_data = 0u32;

        loop {
            Timer::after_millis(300).await;
            let chrono = Instant::now().as_micros();
            let mut sample_count: u8 = 0;

            // ── Read STATUS register ────────────────────────────────────
            match self.sensor.read_status().await {
                Ok(s) => {
                    let eint = (s >> 23) & 1;
                    let eovf = (s >> 22) & 1;
                    let rrint = (s >> 10) & 1;
                    let pllint = (s >> 8) & 1;
                    if pllint != 0 {
                        info!("ECG: PLLINT=1 — PLL not locked!");
                    }
                    if eovf != 0 {
                        info!("ECG: EOVF=1 — FIFO overflow!");
                    }

                    // Brief status every cycle, verbose only when something changes
                    if eint != 0 || eovf != 0 || pllint != 0 {
                        info!(
                            "ECG status: 0x{:06X}  EINT={} EOVF={} RRINT={} PLLINT={}",
                            s, eint, eovf, rrint, pllint
                        );
                    }
                }
                Err(e) => {
                    warn!("ECG: STATUS read error: {}", defmt::Debug2Format(&e));
                    continue;
                }
            }

            // ── Drain FIFO ──────────────────────────────────────────────
            let mut fifo_buffer = [0u8; 96];

            match self.sensor.read_ecg_fifo(&mut fifo_buffer).await {
                Ok(samples_read) => {
                    sample_count = samples_read as u8;

                    if samples_read == 0 {
                        cycles_without_data += 1;
                        if cycles_without_data == 1 || cycles_without_data % 10 == 0 {
                            info!(
                                "ECG: no data in FIFO ({} cycles without)",
                                cycles_without_data
                            );
                        }
                        continue;
                    }
                    cycles_without_data = 0;

                    info!("ECG: read {} samples from FIFO", samples_read);

                    for i in 0..samples_read {
                        let offset = i * 3;

                        // Reconstruct 24-bit FIFO word (MSB first)
                        let raw = ((fifo_buffer[offset] as u32) << 16)
                            | ((fifo_buffer[offset + 1] as u32) << 8)
                            | (fifo_buffer[offset + 2] as u32);

                        // Log first raw sample for debugging
                        if i == 0 {
                            info!(
                                "ECG: first raw word = 0x{:06X} (bytes: {:02X} {:02X} {:02X})",
                                raw,
                                fifo_buffer[offset],
                                fifo_buffer[offset + 1],
                                fifo_buffer[offset + 2]
                            );
                        }

                        // D[23:6] holds the 18-bit ECG sample data (left-justified)
                        let ecg_18bit = (raw >> 6) & 0x3FFFF;

                        // Sign-extend 18-bit two's complement → i32
                        let ecg_sample = if (ecg_18bit & 0x2_0000) != 0 {
                            (ecg_18bit | 0xFFFC_0000) as i32
                        } else {
                            ecg_18bit as i32
                        };

                        // Log first sample value
                        if i == 0 {
                            info!("ECG: first sample = {}", ecg_sample);
                        }

                        // ── DSP pipeline ──────────────────────────────
                        let raw_f32 = ecg_sample as f32;
                        let (dc_blocked, dc_mean) = self.dc_block.process(raw_f32);
                        let clean = self.bandpass.process(dc_blocked);

                        // Log first cycle DC stats
                        if i == 0 && samples_read > 0 {
                            info!(
                                "ECG: DC mean = {=f32}, filtered = {}",
                                dc_mean, clean as i32
                            );
                        }

                        // Push to UI waveform channel
                        let _ = ECG_SAMPLE_BUFFER.try_send(clean as i32);

                        // Feed BPM calculator
                        if let Some(new_bpm) = self.bpm_calc.process_sample(clean) {
                            let smooth_bpm = self.bpm_smoother.process(new_bpm);
                            info!("ECG: HR detected — {} BPM", smooth_bpm as u8);
                            sender.send((smooth_bpm as u8,));
                        }
                    }

                    // ── Read RTOR for hardware HR ────────────────────────
                    match self.sensor.update_heart_rate().await {
                        Ok(Some(hr)) => {
                            info!(
                                "ECG RTOR: {} bpm, RR = {} ms",
                                hr.heart_rate, hr.rr_interval
                            );
                        }
                        Ok(None) => {}
                        Err(e) => {
                            warn!("ECG RTOR read error: {}", defmt::Debug2Format(&e));
                        }
                    }
                }
                Err(e) => {
                    warn!("ECG FIFO read error: {}", defmt::Debug2Format(&e));
                    Timer::after(Duration::from_millis(10)).await;
                }
            }

            let elapsed = Instant::now().as_micros() - chrono;
            info!(
                "ECG: cycle done — {} samples, took {} us",
                sample_count, elapsed
            );
        }
    }
}

// ── Task helper (non‑task, concrete wrapper needed at call site) ──────────────

pub async fn ecg_acquisition_task<SPI, SpiError>(
    runner: EcgRunner<SPI>,
    sender: Sender<'static, CriticalSectionRawMutex, (u8,), 2>,
) -> !
where
    SPI: SpiDevice<Error = SpiError>,
    SpiError: core::fmt::Debug,
{
    runner.run(sender).await;
}

// ── Handle ────────────────────────────────────────────────────────────────────

pub struct EcgHandle;

impl EcgHandle {
    pub async fn start<SPI, D, SpiError>(mut sensor: Max30003<SPI>, delay: &mut D) -> EcgRunner<SPI>
    where
        SPI: SpiDevice<Error = SpiError>,
        D: DelayNs,
        SpiError: core::fmt::Debug,
    {
        info!("ECG: initializing sensor...");
        sensor.begin(delay).await.unwrap();
        info!("ECG: begin() done, calling sync()...");
        sensor.sync().await.unwrap();
        info!("ECG: sync() done, runner ready");

        EcgRunner {
            sensor,
            dc_block: MovingMeanSubtractor::new(),
            bandpass: FirFilter::new(FIR_COEFFS),
            bpm_calc: BpmCalculator::new(128.0),
            bpm_smoother: Smoother::new(0.20, 0.40),
        }
    }
}
