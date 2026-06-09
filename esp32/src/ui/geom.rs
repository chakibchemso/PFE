//! Shared UI geometry — all hardcoded pixel values in the UI codebase are
//! scaled from their original 466×466 design to the current logical
//! resolution via [`scale`].  When `SCREEN_W` / `SCREEN_H` change (e.g.
//! to 466 for full-res), every widget automatically re-sizes.
//!
//! Use `scale(val)` for any absolute pixel value that was written for the
//! full-resolution display.  Use `CX` / `CY` for centering.

use crate::services::rendering::task::{SCREEN_H, SCREEN_W};
use crate::ui::config::PRODUCTION_UI_SIZE;

/// Scale a value designed for `PRODUCTION_UI_SIZE` (466) to the current
/// logical resolution (233).  E.g. `scale(200)` → 100.
/// Integer-safe: `v * SCREEN_W / PRODUCTION_UI_SIZE`.
#[inline]
pub const fn scale(v: i32) -> i32 {
    v * SCREEN_W / PRODUCTION_UI_SIZE as i32
}

/// Percentage of screen width (0..100).  Useful for widths / sizes that
/// should always occupy a fixed fraction of the display.
#[inline]
pub const fn w_pct(pct: i32) -> i32 {
    SCREEN_W * pct / 100
}

/// Percentage of screen height (0..100).
#[inline]
pub const fn h_pct(pct: i32) -> i32 {
    SCREEN_H * pct / 100
}

/// Screen centre in logical pixels.
pub const CX: i32 = SCREEN_W / 2;
pub const CY: i32 = SCREEN_H / 2;
