use defmt::{error, info, warn};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Sender;
use embassy_time::{Duration, Timer};
use nmea::sentences::FixType;
use nmea::{Nmea, SentenceType};

use crate::app::bus::GpsFix;
use crate::drivers::bus::I2cPeripheral;
use crate::drivers::gps::Lc76g;

#[embassy_executor::task]
pub async fn gps_task(
    i2c: I2cPeripheral,
    sender: Sender<'static, CriticalSectionRawMutex, Option<GpsFix>, 2>,
) {
    let mut gps = Lc76g::new(i2c);
    let mut nmea = Nmea::create_for_navigation(&[SentenceType::RMC, SentenceType::GGA]).unwrap();

    // --- Power on sequence ---
    // info!("GPS: sending PAIR002 (power on)...");
    // match gps.send_command(b"$PAIR002*38\r\n").await {
    //     Ok(_) => info!("GPS: PAIR002 sent OK"),
    //     Err(e) => warn!("GPS: PAIR002 send failed: {}", defmt::Debug2Format(&e)),
    // }

    // Cold start — retry a few times; the I2C bridge sometimes NACKs
    // the first write while processing PAIR002's ACKs.
    // Timer::after(Duration::from_millis(500)).await;
    // for attempt in 0u8..3 {
    //     info!("GPS: sending PAIR006 cold start (attempt {})...", attempt + 1);
    //     match gps.send_command(b"$PAIR006*3C\r\n").await {
    //         Ok(_) => {
    //             info!("GPS: PAIR006 sent OK");
    //             break;
    //         }
    //         Err(e) if attempt < 2 => {
    //             warn!("GPS: PAIR006 failed, retrying: {}", defmt::Debug2Format(&e));
    //             Timer::after(Duration::from_millis(500)).await;
    //         }
    //         Err(e) => {
    //             warn!("GPS: PAIR006 failed after 3 attempts: {}", defmt::Debug2Format(&e));
    //         }
    //     }
    // }

    info!("GPS task entering poll loop");

    let mut was_fixed = false;
    let mut parsed: u32 = 0;
    let mut skipped: u32 = 0;
    let mut failed: u32 = 0;
    let mut last_diag: u32 = 0;
    let mut poll_count: u32 = 0;

    loop {
        let blob = match gps.poll().await {
            Ok(Some(data)) => data,
            Ok(None) => {
                Timer::after(Duration::from_millis(500)).await;
                continue;
            }
            Err(e) => {
                error!("GPS: I2C error: {}", defmt::Debug2Format(&e));
                Timer::after(Duration::from_millis(500)).await;
                continue;
            }
        };

        poll_count += 1;
        let do_dump = poll_count % 60 == 1;

        // Collect all lines once
        let lines: [_; 64] = {
            let mut arr: [Option<&str>; 64] = [None; 64];
            let mut i = 0;
            for raw_line in blob.split(|&b| b == b'\n') {
                if i >= 64 {
                    break;
                }
                let line: &[u8] = if raw_line.ends_with(b"\r") {
                    &raw_line[..raw_line.len() - 1]
                } else {
                    raw_line
                };
                if let Ok(s) = core::str::from_utf8(line) {
                    if !s.is_empty() {
                        arr[i] = Some(s);
                        i += 1;
                    }
                }
            }
            arr
        };

        if do_dump {
            info!("--- GPS raw dump (poll {}) ---", poll_count);
            for line in lines.iter().flatten() {
                info!("  {}", *line);
            }
            info!("--- end dump ---");
        }

        for sentence in lines.iter().flatten() {
            let s = *sentence;

            // Skip non-NMEA and proprietary ($P) and overlong
            if !s.starts_with('$') {
                continue;
            }
            if s.as_bytes().get(1) == Some(&b'P') || s.len() > 100 {
                skipped += 1;
                continue;
            }

            match nmea.parse(s) {
                Ok(_) => parsed += 1,
                Err(_) => failed += 1,
            }
        }

        let has_fix = nmea.fix_type.map(|ft| ft.is_valid()).unwrap_or(false);
        let total = parsed + skipped + failed;
        let sats_in_view = nmea.satellites().len() as u8;

        // Log fix transition
        if has_fix != was_fixed {
            if has_fix {
                info!(
                    "GPS: FIX ACQUIRED! sats_used={}, lat={}, lon={}, alt={}",
                    nmea.num_of_fix_satellites.unwrap_or(0),
                    nmea.latitude.unwrap_or(0.0),
                    nmea.longitude.unwrap_or(0.0),
                    nmea.altitude.unwrap_or(0.0),
                );
            } else {
                warn!("GPS: fix lost");
            }
            was_fixed = has_fix;
        }

        // Diagnostic dump every ~60 sentences (~30s at 1 Hz with multiple sentence types)
        if total - last_diag >= 60 {
            info!(
                "GPS diag: fix={}, sats_used={}, sats_view={}, lat={}, lon={}, alt={}, hdop={}",
                has_fix,
                nmea.num_of_fix_satellites.unwrap_or(0),
                sats_in_view,
                nmea.latitude.unwrap_or(0.0),
                nmea.longitude.unwrap_or(0.0),
                nmea.altitude.unwrap_or(0.0),
                nmea.hdop.unwrap_or(0.0),
            );
            info!(
                "GPS sentences: {} parsed, {} skipped, {} failed",
                parsed, skipped, failed,
            );
            last_diag = total;
        }

        let fix = GpsFix {
            lat: nmea.latitude.unwrap_or(0.0) as f32,
            lon: nmea.longitude.unwrap_or(0.0) as f32,
            speed_kmh: nmea.speed_over_ground.unwrap_or(0.0) * 1.852,
            heading_deg: nmea.true_course.unwrap_or(0.0),
            altitude_m: nmea.altitude.unwrap_or(0.0),
            satellites: sats_in_view,
            has_fix,
            fix_quality: match nmea.fix_type {
                Some(FixType::Invalid) => 0,
                Some(FixType::Gps) => 1,
                Some(FixType::DGps) => 2,
                Some(FixType::Pps) => 3,
                Some(FixType::Rtk) => 4,
                Some(FixType::FloatRtk) => 5,
                Some(FixType::Estimated) => 6,
                Some(FixType::Manual) => 7,
                Some(FixType::Simulation) => 8,
                None => 0,
            },
        };

        sender.send(Some(fix));

        Timer::after(Duration::from_millis(500)).await;
    }
}
