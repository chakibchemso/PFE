use alloc::string::String;
use esp_hal::rng::Rng;
use core::fmt::Write;

// Helper to avoid writing `static` variables manually
#[macro_export]
macro_rules! mk_static {
    ($t:ty, $val:expr) => {
        {
        static STATIC_CELL: StaticCell<$t> = StaticCell::new();
        #[deny(unused_attributes)]
        let x = STATIC_CELL.uninit().write(($val));
        x
        }
    };
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
