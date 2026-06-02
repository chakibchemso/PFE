//! Tileview-based watch UI layout — creates 5 panes and wires theme switching.

use lv_bevy_ecs::sys::{lv_dir_t_LV_DIR_LEFT, lv_dir_t_LV_DIR_RIGHT, lv_obj_t};
use lv_bevy_ecs::widgets::Tileview;

use super::theme::{self, current_palette};
use super::{ecg, gps, settings, vitals, watchface};

/// Aggregated handles for all UI widgets that need runtime updates.
pub struct AppHandles {
    pub panes: [*mut lv_obj_t; 5],
    pub settings: settings::Handles,
    pub watchface: watchface::Handles,
    pub vitals: vitals::Handles,
}

/// Create the full tileview UI: 5 panes spanning a horizontal strip.
pub fn create_tileview() -> AppHandles {
    let mut tv = Tileview::new();
    settings::set_tileview_handle(tv.raw_mut());

    // Pane 0: Settings
    let mut p0 = tv
        .add_tile(0, 1, lv_dir_t_LV_DIR_RIGHT)
        .expect("tileview add_tile(0,0)");

    // Pane 1: Watchface (center)
    let mut p1 = tv
        .add_tile(1, 1, lv_dir_t_LV_DIR_LEFT | lv_dir_t_LV_DIR_RIGHT)
        .expect("tileview add_tile(1,0)");

    // Pane 2: Vitals
    let mut p2 = tv
        .add_tile(2, 1, lv_dir_t_LV_DIR_LEFT | lv_dir_t_LV_DIR_RIGHT)
        .expect("tileview add_tile(2,0)");

    // Pane 3: ECG
    let mut p3 = tv
        .add_tile(3, 1, lv_dir_t_LV_DIR_LEFT | lv_dir_t_LV_DIR_RIGHT)
        .expect("tileview add_tile(3,0)");

    // Pane 4: GPS
    let mut p4 = tv
        .add_tile(4, 1, lv_dir_t_LV_DIR_LEFT)
        .expect("tileview add_tile(4,0)");

    let settings_h = settings::create(&mut p0);
    let watchface_h = watchface::create(&mut p1);
    let vitals_h = vitals::create(&mut p2);
    ecg::create(&mut p3);
    gps::create(&mut p4);

    // Save pane raw pointers for re-theming
    let panes = [
        p0.raw_mut(),
        p1.raw_mut(),
        p2.raw_mut(),
        p3.raw_mut(),
        p4.raw_mut(),
    ];

    // Start on watchface
    tv.set_tile_by_index(1, 1, false);

    // Initial theme
    let pal = current_palette();
    for pane in &panes {
        theme::apply_to_pane(*pane, pal);
    }

    let _ = tv.leak();

    AppHandles {
        panes,
        settings: settings_h,
        watchface: watchface_h,
        vitals: vitals_h,
    }
}

/// Re-apply theme across all tiles and watchface/vitals-specific widgets.
pub fn apply_theme(h: &AppHandles) {
    let pal = current_palette();
    for pane in &h.panes {
        theme::apply_to_pane(*pane, pal);
    }
    watchface::apply_theme(&h.watchface, pal);
    vitals::apply_theme(&h.vitals, pal);
}
