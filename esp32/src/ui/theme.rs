use core::sync::atomic::{AtomicU8, Ordering};

#[derive(Clone, Copy)]
pub struct ThemePalette {
    pub bg_color: u32,
    pub text_color: u32,
    pub surface_color: u32,
    pub overlay_color: u32,
    pub accent_color: u32,
}

pub const LATTE: ThemePalette = ThemePalette {
    bg_color: 0xeff1f5,
    text_color: 0x4c4f69,
    surface_color: 0xe6e9ef,
    overlay_color: 0x9ca0b0,
    accent_color: 0x1e66f5,
};

pub const MOCHA: ThemePalette = ThemePalette {
    bg_color: 0x1e1e2e,
    text_color: 0xcdd6f4,
    surface_color: 0x313244,
    overlay_color: 0x6c7086,
    accent_color: 0x89b4fa,
};

pub static CURRENT_THEME: AtomicU8 = AtomicU8::new(1);

pub fn current_palette() -> &'static ThemePalette {
    match CURRENT_THEME.load(Ordering::Relaxed) {
        0 => &LATTE,
        _ => &MOCHA,
    }
}
