//! Board-level hardware configuration: pin mappings, SPI/I2C ports, DMA channels.
//!
//! # Production Board (PicoPump v1)
//!
//! ## AMOLED Display (QSPI, CO5300)
//! | Signal    | GPIO  |
//! |-----------|-------|
//! | LCD_CS    | GPIO12|
//! | QSPI_SCLK | GPIO38|
//! | QSPI_SIO0 | GPIO4 |
//! | QSPI_SIO1 | GPIO5 |
//! | QSPI_SIO2 | GPIO6 |
//! | QSPI_SIO3 | GPIO7 |
//! | LCD_TE    | GPIO13|
//! | LCD_RST   | GPIO39|
//!
//! ## Touch Panel (I2C, CST9217)
//! | Signal  | GPIO  |
//! |---------|-------|
//! | TP_SDA  | GPIO15|
//! | TP_SCL  | GPIO14|
//! | TP_INT  | GPIO11|
//! | TP_RST  | GPIO40|
//!
//! ## GPS Module (I2C, LC76G)
//! | Signal  | GPIO  |
//! |---------|-------|
//! | GPS_SDA | GPIO15|  (shared I2C bus)
//! | GPS_SCL | GPIO14|  (shared I2C bus)

use esp_hal::{
    Async,
    dma::{DmaRxBuf, DmaTxBuf},
    dma_descriptors,
    gpio::{Input, InputConfig, Level, Output, OutputConfig, Pull},
    i2c::master::{BusTimeout, Config as I2cConfig, I2c},
    peripherals,
    psram::{FlashFreq, PsramConfig, PsramMode, PsramSize, SpiRamFreq, SpiTimingConfigCoreClock},
    spi::master::{Config as SpiConfig, Spi, SpiDma},
    time::Rate,
};

use core::mem::MaybeUninit;
use static_cell::StaticCell;

/// DMA buffer size for SPI transfers (under 19-bit max, divisible by 4 and 32).
pub const SPI_DMA_RX_SIZE: usize = 64; // unused
pub const SPI_DMA_TX_SIZE: usize = 32736 / 4; // 8192

// DMA buffers placed in .bss (zeroed by CRT before main).
// Avoid mk_static!([0; SIZE]) — that creates a SIZE-byte temporary on the
// stack, and two 32KB temps eat 64KB of core 0's ~102KB stack.
static DMA_RX_BUF: StaticCell<MaybeUninit<[u8; SPI_DMA_RX_SIZE]>> = StaticCell::new();
static DMA_TX_BUF: StaticCell<MaybeUninit<[u8; SPI_DMA_TX_SIZE]>> = StaticCell::new();

/// SPI clock frequency (40 MHz — safe for CO5300 init; can bump to 80 MHz later)
pub const SPI_FREQ_MHZ: u32 = 80;

/// I2C clock frequency (400 kHz standard)
pub const I2C_FREQ_KHZ: u32 = 400;

/// Initialized board peripherals for the production board
pub struct BoardPeripherals {
    pub i2c_bus: I2c<'static, Async>,
    pub qspi_spi: SpiDma<'static, Async>,
    pub qspi_rx: DmaRxBuf,
    pub qspi_tx: DmaTxBuf,
    pub display_cs: Output<'static>,
    pub display_rst: Output<'static>,
    pub display_te: Input<'static>,
    pub touch_rst: Output<'static>,
    pub touch_int: Input<'static>,
}

/// Initialize board-level peripherals for the production CO5300/CST9217 board.
///
/// ## Consumed peripherals
/// - SPI2, DMA_CH0: QSPI display bus with DMA
/// - I2C0: shared bus for touch, GPS, and sensors
/// - GPIO14, GPIO15: I2C SCL/SDA
/// - GPIO4, GPIO5, GPIO6, GPIO7, GPIO12, GPIO38: QSPI data/control
/// - GPIO13: display TE (tearing effect / VSYNC input)
/// - GPIO39: display reset
/// - GPIO11, GPIO40: touch INT and reset
#[allow(clippy::too_many_arguments)]
pub fn init_board(
    psram: peripherals::PSRAM<'static>,
    spi2: peripherals::SPI2<'static>,
    dma_ch0: peripherals::DMA_CH0<'static>,
    i2c0: peripherals::I2C0<'static>,
    gpio4: peripherals::GPIO4<'static>,
    gpio5: peripherals::GPIO5<'static>,
    gpio6: peripherals::GPIO6<'static>,
    gpio7: peripherals::GPIO7<'static>,
    gpio11: peripherals::GPIO11<'static>,
    gpio12: peripherals::GPIO12<'static>,
    gpio13: peripherals::GPIO13<'static>,
    gpio14: peripherals::GPIO14<'static>,
    gpio15: peripherals::GPIO15<'static>,
    gpio38: peripherals::GPIO38<'static>,
    gpio39: peripherals::GPIO39<'static>,
    gpio40: peripherals::GPIO40<'static>,
) -> BoardPeripherals {
    // Initialize heap: DRAM for WiFi DMA + PSRAM for everything else.
    // WiFi hardware DMA cannot address PSRAM — its buffers MUST land in
    // internal SRAM. Reserve a DRAM pool just large enough for WiFi.
    {
        esp_alloc::heap_allocator!(size: 32 * 1024);
        esp_alloc::heap_allocator!(#[esp_hal::ram(reclaimed)] size: 73744); // 73744
        let psram_config = PsramConfig {
            mode: PsramMode::OctalSpi,
            size: PsramSize::AutoDetect,
            core_clock: Some(SpiTimingConfigCoreClock::SpiTimingConfigCoreClock160m),
            flash_frequency: FlashFreq::FlashFreq80m,
            ram_frequency: SpiRamFreq::Freq80m,
        };
        esp_alloc::psram_allocator!(psram, esp_hal::psram, psram_config);
    }

    // --- Display control pins ---
    let display_cs = Output::new(gpio12, Level::High, OutputConfig::default());
    let display_rst = Output::new(gpio39, Level::Low, OutputConfig::default());

    // TE pin: input with pull-down; display drives this high at VSYNC start
    let display_te = Input::new(gpio13, InputConfig::default().with_pull(Pull::Down));

    // --- Touch control pins ---
    let touch_rst = Output::new(gpio40, Level::Low, OutputConfig::default());
    let touch_int = Input::new(gpio11, InputConfig::default().with_pull(Pull::Up));

    // --- QSPI setup (SPI2 with quad I/O lines) ---
    let (rx_descriptors, tx_descriptors) = dma_descriptors!(SPI_DMA_RX_SIZE, SPI_DMA_TX_SIZE);
    // Buffer memory is in .bss — already zeroed by CRT. No stack temporary.
    let dma_rx_buf = DmaRxBuf::new(rx_descriptors, unsafe {
        DMA_RX_BUF.init(MaybeUninit::uninit()).assume_init_mut()
    })
    .unwrap();

    let dma_tx_buf = DmaTxBuf::new(tx_descriptors, unsafe {
        DMA_TX_BUF.init(MaybeUninit::uninit()).assume_init_mut()
    })
    .unwrap();

    let qspi_bus = Spi::new(
        spi2,
        SpiConfig::default()
            .with_frequency(Rate::from_mhz(SPI_FREQ_MHZ))
            .with_mode(esp_hal::spi::Mode::_0),
    )
    .expect("Failed to initialize QSPI")
    .with_sck(gpio38)
    .with_sio0(gpio4) // SIO0
    .with_sio1(gpio5) // SIO1
    .with_sio2(gpio6) // SIO2
    .with_sio3(gpio7) // SIO3
    .with_dma(dma_ch0)
    .with_buffers(dma_rx_buf, dma_tx_buf)
    .into_async();

    // Single device on this bus — split immediately. The display driver owns
    // SpiDma directly and uses native half_duplex_write for QSPI transfers.
    let (qspi_spi, qspi_rx, qspi_tx) = qspi_bus.split();

    // --- I2C setup (shared bus for touch + GPS + sensors) ---
    let i2c_bus = I2c::new(
        i2c0,
        I2cConfig::default()
            .with_frequency(Rate::from_khz(I2C_FREQ_KHZ))
            // Fail fast on a held-low SCL/SDA instead of letting one sensor
            // transaction wedge the shared async I2C bus indefinitely.
            .with_timeout(BusTimeout::BusCycles(8_000)),
    )
    .expect("Failed to initialize I2C")
    .with_scl(gpio14)
    .with_sda(gpio15)
    .into_async();

    BoardPeripherals {
        i2c_bus,
        qspi_spi,
        qspi_rx,
        qspi_tx,
        display_cs,
        display_rst,
        display_te,
        touch_rst,
        touch_int,
    }
}
