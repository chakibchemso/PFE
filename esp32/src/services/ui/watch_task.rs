use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Receiver;
use embassy_time::Timer;
use lv_bevy_ecs::sys::*;

use crate::ui::watchface::Handles;
use crate::utils::SendWrap;

type UtcReceiver = Receiver<'static, CriticalSectionRawMutex, u64, 2>;

/// Local time offset from UTC in seconds. Positive = East of UTC.
/// GMT+1 = 3600 seconds.
const TZ_OFFSET_SECS: i64 = 3600;

#[embassy_executor::task]
pub async fn watch_task(
    handles: SendWrap<Handles>,
    mut utc_rx: Option<UtcReceiver>,
) -> ! {
    Timer::after_millis(500).await;
    let h = handles.0;

    let mut epoch = if let Some(ref mut rx) = utc_rx {
        rx.changed().await
    } else {
        0
    };

    loop {
        Timer::after_secs(1).await;

        if let Some(ref mut rx) = utc_rx {
            if let Some(new_epoch) = rx.try_changed() {
                epoch = new_epoch;
            } else {
                epoch += 1;
            }
        } else {
            epoch += 1;
        }

        let local = (epoch as i64 + TZ_OFFSET_SECS).rem_euclid(86400) as u64;
        let hours = local / 3600;
        let mins = (local / 60) % 60;
        let secs = local % 60;

        let h_rot = ((hours % 12) * 300 + mins * 5) as i32;
        let m_rot = (mins * 60 + secs) as i32;
        let s_rot = (secs * 60) as i32;

        let mut buf = [0u8; 9];
        buf[0] = b'0' + (hours / 10) as u8;
        buf[1] = b'0' + (hours % 10) as u8;
        buf[2] = b':';
        buf[3] = b'0' + (mins / 10) as u8;
        buf[4] = b'0' + (mins % 10) as u8;
        buf[5] = b':';
        buf[6] = b'0' + (secs / 10) as u8;
        buf[7] = b'0' + (secs % 10) as u8;
        buf[8] = 0;

        unsafe {
            lv_obj_set_style_transform_rotation(h.hour_hand, h_rot, 0);
            lv_obj_set_style_transform_rotation(h.minute_hand, m_rot, 0);
            lv_obj_set_style_transform_rotation(h.second_hand, s_rot, 0);

            lv_label_set_text(
                h.digital_time,
                buf.as_ptr() as *const core::ffi::c_char,
            );
            lv_obj_invalidate(h.digital_time);
        }
    }
}
