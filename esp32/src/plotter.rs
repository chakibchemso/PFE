/// Live signal plotter - compact text protocol for streaming telemetry to Python plotter.
///
/// # Protocol
///
/// All messages are single-line text frames. When printed via `defmt::println!`,
/// the newline is added automatically.
///
/// Frame types:
/// - **Register**: `@R:<id>:<name>:<r>,<g>,<b>`
///   - Example: `@R:0:RawRed:0,200,0`
/// - **Data**: `@D:<id>:<value>`
///   - Example: `@D:0:12345.67`
/// - **Clear**: `@C` — clears all channels
///
/// Channel IDs are assigned automatically on registration (0, 1, 2, ...).
/// Lines not starting with `@` are ignored by the plotter (passthrough for logs).

/// Maximum channel name length
pub const MAX_NAME_LEN: usize = 32;

/// A formatted plot message. Stores the serialized string in a reusable buffer.
pub struct PlotMessage {
    buf: [u8; 128],
    len: usize,
}

impl PlotMessage {
    pub const fn new() -> Self {
        Self {
            buf: [0u8; 128],
            len: 0,
        }
    }

    /// Format a **register** message: `@R:<id>:<name>:<r>,<g>,<b>`
    pub fn register(&mut self, channel_id: u8, name: &str, color: (u8, u8, u8)) -> &str {
        let mut idx = 0;
        self.buf[idx] = b'@';
        idx += 1;
        self.buf[idx] = b'R';
        idx += 1;
        self.buf[idx] = b':';
        idx += 1;

        idx += Self::format_u8_to(channel_id, &mut self.buf[idx..]);

        self.buf[idx] = b':';
        idx += 1;

        let name_bytes = name.as_bytes();
        let name_len = name_bytes.len().min(MAX_NAME_LEN);
        self.buf[idx..idx + name_len].copy_from_slice(&name_bytes[..name_len]);
        idx += name_len;

        self.buf[idx] = b':';
        idx += 1;

        idx += Self::format_u8_to(color.0, &mut self.buf[idx..]);
        self.buf[idx] = b',';
        idx += 1;
        idx += Self::format_u8_to(color.1, &mut self.buf[idx..]);
        self.buf[idx] = b',';
        idx += 1;
        idx += Self::format_u8_to(color.2, &mut self.buf[idx..]);

        self.len = idx;

        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }

    /// Format a **data** message: `@D:<id>:<value>`
    pub fn data(&mut self, channel_id: u8, value: f32) -> &str {
        let mut idx = 0;
        self.buf[idx] = b'@';
        idx += 1;
        self.buf[idx] = b'D';
        idx += 1;
        self.buf[idx] = b':';
        idx += 1;

        idx += Self::format_u8_to(channel_id, &mut self.buf[idx..]);

        self.buf[idx] = b':';
        idx += 1;

        idx += Self::format_f32_to(value, &mut self.buf[idx..]);

        self.len = idx;

        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }

    /// Format a **clear** message: `@C`
    pub fn clear(&mut self) -> &str {
        self.buf[0] = b'@';
        self.buf[1] = b'C';
        self.len = 2;
        "@C"
    }

    /// Format a u8 into a buffer, returning the number of bytes written.
    fn format_u8_to(val: u8, buf: &mut [u8]) -> usize {
        if val == 0 {
            buf[0] = b'0';
            return 1;
        }
        let mut n = val;
        let mut tmp = [0u8; 4];
        let mut i = 0;
        while n > 0 {
            tmp[i] = b'0' + (n % 10);
            n /= 10;
            i += 1;
        }
        for j in 0..i {
            buf[j] = tmp[i - 1 - j];
        }
        i
    }

    /// Format an f32 into a buffer with 2 decimal places, returning bytes written.
    fn format_f32_to(val: f32, buf: &mut [u8]) -> usize {
        let negative = val < 0.0;
        let abs_val = if negative { -val } else { val };
        let scaled = (abs_val * 100.0) as i64;
        let int_part = scaled / 100;
        let frac_part = scaled % 100;

        let mut idx = 0;

        if negative {
            buf[idx] = b'-';
            idx += 1;
        }

        if int_part == 0 {
            buf[idx] = b'0';
            idx += 1;
        } else {
            let mut n = int_part;
            let mut tmp = [0u8; 12];
            let mut i = 0;
            while n > 0 {
                tmp[i] = b'0' + (n % 10) as u8;
                n /= 10;
                i += 1;
            }
            for j in 0..i {
                buf[idx + j] = tmp[i - 1 - j];
            }
            idx += i;
        }

        buf[idx] = b'.';
        idx += 1;

        let frac_abs = if frac_part < 0 { -frac_part } else { frac_part } as u8;
        buf[idx] = b'0' + (frac_abs / 10);
        idx += 1;
        buf[idx] = b'0' + (frac_abs % 10);
        idx += 1;

        idx
    }
}
