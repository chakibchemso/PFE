//! Board-level hardware configuration: pin mappings, SPI/I2C ports, DMA channels.
//! Swap a display or change pins? Edit this file.

use esp_hal::{
    Async,
    dma::{DmaRxBuf, DmaTxBuf},
    dma_descriptors,
    gpio::{Level, Output, OutputConfig},
    i2c::master::{Config as I2cConfig, I2c},
    peripherals,
    spi::{
        Mode,
        master::{Config as SpiConfig, SpiDmaBus},
    },
    time::Rate,
};
use static_cell::StaticCell;

use crate::mk_static;

/// under 19bit max, divisible by 4(spi) and 32(cache). max is 32736
pub const SPI_DMA_RX_SIZE: usize = 32736;
pub const SPI_DMA_TX_SIZE: usize = 32736;

/// SPI clock frequency (80 MHz for ST7796)
pub const SPI_FREQ_MHZ: u32 = 80;

/// I2C clock frequency (400 kHz standard)
pub const I2C_FREQ_KHZ: u32 = 400;

/// Initialized board peripherals
pub struct BoardPeripherals {
    pub i2c_bus: I2c<'static, Async>,
    pub spi_bus: SpiDmaBus<'static, Async>,
    pub display_dc: Output<'static>,
    pub display_rst: Output<'static>,
    pub display_cs: Output<'static>,
}

/// Initialize board-level peripherals for the dev ST7796 board.
///
/// Consumed peripherals (documented to avoid accidental partial-move errors):
/// - SPI2, DMA_CH0: SPI display bus with DMA
/// - I2C0: shared bus for sensors, touch, and future I2C devices
/// - GPIO1, GPIO2: I2C SDA/SCL
/// - GPIO8, GPIO9, GPIO10: display RST, DC, CS
/// - GPIO11, GPIO12: SPI MOSI, SCK
#[allow(clippy::too_many_arguments)]
pub fn init_board(
    spi2: peripherals::SPI2<'static>,
    dma_ch0: peripherals::DMA_CH0<'static>,
    i2c0: peripherals::I2C0<'static>,
    gpio1: peripherals::GPIO1<'static>,
    gpio2: peripherals::GPIO2<'static>,
    gpio8: peripherals::GPIO8<'static>,
    gpio9: peripherals::GPIO9<'static>,
    gpio10: peripherals::GPIO10<'static>,
    gpio11: peripherals::GPIO11<'static>,
    gpio12: peripherals::GPIO12<'static>,
) -> BoardPeripherals {
    // Display control pins
    let display_dc = Output::new(gpio9, Level::Low, OutputConfig::default());
    let display_rst = Output::new(gpio8, Level::Low, OutputConfig::default());
    let display_cs = Output::new(gpio10, Level::High, OutputConfig::default());

    // SPI setup
    let (rx_descriptors, tx_descriptors) = dma_descriptors!(SPI_DMA_RX_SIZE, SPI_DMA_TX_SIZE);
    let dma_rx_buf = DmaRxBuf::new(
        rx_descriptors,
        mk_static!([u8; SPI_DMA_RX_SIZE], [0; SPI_DMA_RX_SIZE]),
    )
    .unwrap();

    let dma_tx_buf = DmaTxBuf::new(
        tx_descriptors,
        mk_static!([u8; SPI_DMA_TX_SIZE], [0; SPI_DMA_TX_SIZE]),
    )
    .unwrap();

    let spi_bus = esp_hal::spi::master::Spi::new(
        spi2,
        SpiConfig::default()
            .with_frequency(Rate::from_mhz(SPI_FREQ_MHZ))
            .with_mode(Mode::_0),
    )
    .expect("Failed to initialize SPI")
    .with_sck(gpio12)
    .with_mosi(gpio11)
    .with_dma(dma_ch0)
    .with_buffers(dma_rx_buf, dma_tx_buf)
    .into_async();

    // I2C setup — single shared bus for all I2C devices
    let i2c_bus = I2c::new(
        i2c0,
        I2cConfig::default().with_frequency(Rate::from_khz(I2C_FREQ_KHZ)),
    )
    .expect("Failed to initialize I2C")
    .with_sda(gpio1)
    .with_scl(gpio2)
    .into_async();

    BoardPeripherals {
        i2c_bus,
        spi_bus,
        display_dc,
        display_rst,
        display_cs,
    }
}
