use alloc::string::String;
use core::fmt::Write;
use defmt::info;
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
