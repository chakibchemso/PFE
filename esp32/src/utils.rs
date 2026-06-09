use alloc::string::String;
use core::cell::RefCell;
use core::fmt::Write;
use defmt::info;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_time::Instant;
use embedded_graphics::{
    pixelcolor::{Rgb565, Rgb888},
    prelude::DrawTarget,
};
use esp_hal::rng::Rng;

// Helper to avoid writing `static` variables manually
#[macro_export]
macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: StaticCell<$t> = StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
    }};
}

/// Wrapper to move `!Send` peripherals across CPU cores on ESP32.
///
/// esp-hal marks most driver types `!Send` as a safety precaution, but on
/// ESP32 multi-core chips all peripherals are globally addressable from both
/// cores. This wrapper is the single `unsafe` bridge for cross-core moves.
pub struct SendWrap<T>(pub T);
unsafe impl<T> Send for SendWrap<T> {}

/// Named wall-clock performance counter.
///
/// Thread-safe, fixed capacity `N` (const generic — statically allocated).
/// Each name lives in two internal arrays: one pending start (`u64` μs) and
/// one completed elapsed value (`u32` μs).
///
/// # Usage
/// ```ignore
/// static PERF: PerfCounter<8> = PerfCounter::new();
///
/// PERF.start("render", Instant::now());
/// //  … work …
/// PERF.stop("render", Instant::now());
///
/// defmt::info!("render took {} μs", PERF.get("render"));
///
/// // Total wall-clock since start:
/// let total = PERF.since_start("frame", Instant::now());
/// ```
pub struct PerfCounter<const N: usize> {
    inner: Mutex<CriticalSectionRawMutex, RefCell<PerfInner<N>>>,
}

struct PerfInner<const N: usize> {
    starts: [Option<(&'static str, u64)>; N],
    values: [Option<(&'static str, u32)>; N],
}

impl<const N: usize> PerfInner<N> {
    const fn new() -> Self {
        Self {
            starts: [None; N],
            values: [None; N],
        }
    }
}

impl<const N: usize> PerfCounter<N> {
    pub const fn new() -> Self {
        Self {
            inner: Mutex::new(RefCell::new(PerfInner::new())),
        }
    }

    /// Record the start instant of a named measurement.
    /// Overwrites a previous start with the same name; otherwise takes
    /// the first free slot.  Silently drops if all slots are occupied.
    pub fn start(&self, name: &'static str, instant: Instant) {
        let micros = instant.as_micros();
        self.inner.lock(|inner| {
            let inner = &mut *inner.borrow_mut();
            for slot in inner.starts.iter_mut() {
                if let Some((n, _)) = slot {
                    if *n == name {
                        *slot = Some((name, micros));
                        return;
                    }
                }
            }
            for slot in inner.starts.iter_mut() {
                if slot.is_none() {
                    *slot = Some((name, micros));
                    return;
                }
            }
        });
    }

    /// Stop a named measurement and store the elapsed μs.
    /// The elapsed value can be read with [`get`](Self::get).
    /// If no matching start exists this is a no-op.
    pub fn stop(&self, name: &'static str, instant: Instant) {
        let micros = instant.as_micros();
        self.inner.lock(|inner| {
            let inner = &mut *inner.borrow_mut();
            let start = inner.starts.iter_mut().find_map(|slot| {
                if let Some((n, s)) = slot {
                    if *n == name {
                        let s = *s;
                        *slot = None;
                        Some(s)
                    } else {
                        None
                    }
                } else {
                    None
                }
            });
            let Some(start) = start else { return };

            let elapsed = micros.wrapping_sub(start) as u32;
            for slot in inner.values.iter_mut() {
                if let Some((n, _)) = slot {
                    if *n == name {
                        *slot = Some((name, elapsed));
                        return;
                    }
                }
            }
            for slot in inner.values.iter_mut() {
                if slot.is_none() {
                    *slot = Some((name, elapsed));
                    return;
                }
            }
        });
    }

    /// Directly store a named `u32` value (e.g. a frame counter).
    pub fn set(&self, name: &'static str, value: u32) {
        self.inner.lock(|inner| {
            let inner = &mut *inner.borrow_mut();
            for slot in inner.values.iter_mut() {
                if let Some((n, _)) = slot {
                    if *n == name {
                        *slot = Some((name, value));
                        return;
                    }
                }
            }
            for slot in inner.values.iter_mut() {
                if slot.is_none() {
                    *slot = Some((name, value));
                    return;
                }
            }
        });
    }

    /// Read a stored elapsed or stamped value by name.
    /// Returns 0 when the name was never stored.
    pub fn get(&self, name: &'static str) -> u32 {
        self.inner.lock(|inner| {
            let inner = inner.borrow();
            inner
                .values
                .iter()
                .find_map(|slot| {
                    if let Some((n, v)) = slot {
                        if *n == name { Some(*v) } else { None }
                    } else {
                        None
                    }
                })
                .unwrap_or(0)
        })
    }

    /// μs since the last [`start`](Self::start) for `name`.
    /// Useful for total wall-clock time of an ongoing measurement.
    pub fn since_start(&self, name: &'static str, now: Instant) -> u32 {
        let now = now.as_micros();
        self.inner.lock(|inner| {
            let inner = inner.borrow();
            inner
                .starts
                .iter()
                .find_map(|slot| {
                    if let Some((n, s)) = slot {
                        if *n == name {
                            Some(now.wrapping_sub(*s) as u32)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .unwrap_or(0)
        })
    }
}

pub fn custom_getrandom(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    Rng::new().read(buf);
    Ok(())
}

pub fn print_hex(bytes: &[u8]) -> String {
    let mut hex_str = String::new();
    for byte in bytes {
        write!(&mut hex_str, "{:02X}", byte).unwrap();
    }
    hex_str
}

/// Ultra-fast, integer-only color wheel.
/// Pass a value from 0 to 255 to sweep through the entire color spectrum.
pub fn color_wheel(step: u8) -> Rgb565 {
    let pos = 255 - step;
    let (r, g, b) = if pos < 85 {
        (255 - pos * 3, 0, pos * 3)
    } else if pos < 170 {
        let p = pos - 85;
        (0, p * 3, 255 - p * 3)
    } else {
        let p = pos - 170;
        (p * 3, 255 - p * 3, 0)
    };

    // Rgb888 uses 0-255 channels, which converts perfectly down to 16-bit Rgb565
    Rgb565::from(Rgb888::new(r, g, b))
}

use embassy_time::{Duration, Ticker};

pub async fn fade_disp_colors<D>(display: &mut D)
where
    D: DrawTarget<Color = Rgb565>,
    D::Error: core::fmt::Debug,
{
    let mut color_step: u8 = 0;

    // Ticker is better than Timer::after because it won't drift over time!
    let mut ticker = Ticker::every(Duration::from_millis(33)); // 30 FPS smooth fade

    info!("Starting Display Task!");

    loop {
        let current_color = color_wheel(color_step);
        display.clear(current_color).unwrap();

        color_step = color_step.wrapping_add(2);

        if color_step == 0 {
            info!("Completed a full color cycle!");
            break;
        }

        ticker.next().await;
    }
}
