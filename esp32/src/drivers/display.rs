//! Display initialization: SPI bus wrapper, lcd-async init, orientation.

use embassy_time::Delay;
use esp_hal::Async;
use esp_hal::gpio::Output;
use esp_hal::spi::master::SpiDmaBus;
use lcd_async::options::{ColorInversion, ColorOrder, Orientation};
use lcd_async::{Builder, interface::SpiInterface, models::ST7796};
use static_cell::StaticCell;

use crate::mk_static;
use crate::ui;

/// Initialize the display from an SPI bus and control pins.
pub async fn init_display(
    spi_bus: SpiDmaBus<'static, Async>,
    dc: Output<'static>,
    rst: Output<'static>,
    cs: Output<'static>,
) -> ui::SmartWatchDisplay {
    // Async SpiDevice wrapper for shared bus access
    let spi_bus = mk_static!(ui::DisplaySpiBus, ui::DisplaySpiBus::new(spi_bus));
    let spi_dev = ui::DisplaySpiDevice::new(spi_bus, cs);

    // Initialize the display via lcd-async SpiInterface
    let di = SpiInterface::new(spi_dev, dc);
    let mut delay = Delay;
    let mut display = Builder::new(ST7796, di)
        .reset_pin(rst)
        .color_order(ColorOrder::Bgr)
        .invert_colors(ColorInversion::Inverted)
        .init(&mut delay)
        .await
        .unwrap();

    let render_config = ui::RenderConfig::dev_st7796();
    let mut display_orientation = Orientation::default();
    if render_config.display_mirror_x {
        display_orientation = display_orientation.flip_horizontal();
    }
    if render_config.display_mirror_y {
        display_orientation = display_orientation.flip_vertical();
    }
    display.set_orientation(display_orientation).await.unwrap();

    display
}
