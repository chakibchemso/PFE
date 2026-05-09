//! Display initialization and type aliases for the production CO5300 board.

use alloc::vec;
use alloc::vec::Vec;
use defmt::info;
use display_driver::panel::reset::LCDResetOption;
use display_driver::{ColorFormat, DisplayDriver, Orientation};
use display_driver_co5300::Co5300;
use display_driver_co5300::spec::AM151Q466466LK_151_C;
use embassy_time::Delay;
use esp_hal::gpio::Output;
use esp_hal::spi::master::SpiDmaBus;

use crate::drivers::qspi_bus::SpiDisplayBus;
use crate::ui::config::PRODUCTION_UI_SIZE;

/// Full bus: raw QSPI, no TE gating.
pub type ProductionBus = SpiDisplayBus;

/// The CO5300 panel driver for the 466x466 AM151 panel.
pub type ProductionPanel = Co5300<AM151Q466466LK_151_C, Output<'static>, ProductionBus>;

/// Top-level display handle used by the rendering pipeline.
pub type SmartWatchDisplay = DisplayDriver<ProductionBus, ProductionPanel>;

/// Construct and initialise the CO5300 AMOLED display.
pub async fn init_display(
    spi_bus: SpiDmaBus<'static, esp_hal::Async>,
    cs: Output<'static>,
    rst: Output<'static>,
) -> SmartWatchDisplay {
    info!("display: building bus...");

    let qspi = SpiDisplayBus::new(spi_bus, cs);
    let reset = LCDResetOption::PinLow(rst);
    let panel: Co5300<AM151Q466466LK_151_C, Output<'static>, _> = Co5300::new(reset);

    let mut display = DisplayDriver::new(qspi, panel);
    let mut delay = Delay;

    info!("display: running init sequence...");
    display
        .init(&mut delay)
        .await
        .expect("CO5300 display init failed");
    info!("display: init OK");

    display
        .set_color_format(ColorFormat::RGB565)
        .await
        .expect("set_color_format failed");
    info!("display: colour format set to RGB565");

    display
        .set_orientation(Orientation::Deg0)
        .await
        .expect("set_orientation failed");
    info!("display: orientation set");

    display
        .set_brightness(255)
        .await
        .expect("set_brightness failed");
    info!("display: brightness set to 255");

    // Diagnostic: fill screen with red
    let red_bytes: Vec<u8> = vec![0xF8u8, 0x00u8]
        .repeat((PRODUCTION_UI_SIZE as usize * PRODUCTION_UI_SIZE as usize) as usize);
    let area = display_driver::Area::from_origin(PRODUCTION_UI_SIZE, PRODUCTION_UI_SIZE);
    let frame_ctrl = display_driver::bus::FrameControl::new_standalone();
    match display.write_pixels(area, frame_ctrl, &red_bytes).await {
        Ok(()) => info!("display: diagnostic RED fill OK"),
        Err(_) => info!("display: diagnostic RED fill FAILED"),
    }

    info!("display: ready");
    display
}
