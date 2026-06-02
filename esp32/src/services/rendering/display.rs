//! Display initialization and type aliases for the production CO5300 board.

use alloc::vec;
use alloc::vec::Vec;
use defmt::info;
use display_driver::DisplayError;
use display_driver::bus::{DisplayBus, ErrorType, Metadata};
use display_driver::panel::reset::LCDResetOption;
use display_driver::{ColorFormat, DisplayDriver, Orientation};
use display_driver_co5300::Co5300;
use display_driver_co5300::spec::AM151Q466466LK_151_C;
use embassy_time::{Delay, Timer};
use esp_hal::Async;
use esp_hal::dma::{DmaRxBuf, DmaTxBuf};
use esp_hal::gpio::Output;
use esp_hal::spi::master::{Address, Command, DataMode, SpiDma};

use crate::ui::config::PRODUCTION_UI_SIZE;

/// DMA transfer chunk — matches the TX buffer capacity (SPI_DMA_TX_SIZE).
const DMA_CHUNK: usize = 8184; // 32736 / 4

/// Owns the `SpiDma` peripheral directly — no `SpiDmaBus` wrapper needed
/// since the display is the only device on this QSPI bus.
pub struct QspiDisplayBus {
    spi: Option<SpiDma<'static, Async>>,
    #[allow(dead_code)]
    rx_buf: DmaRxBuf,
    tx_buf: Option<DmaTxBuf>,
    cs: Output<'static>,
}

impl QspiDisplayBus {
    pub fn new(
        spi: SpiDma<'static, Async>,
        rx_buf: DmaRxBuf,
        tx_buf: DmaTxBuf,
        cs: Output<'static>,
    ) -> Self {
        Self {
            spi: Some(spi),
            rx_buf,
            tx_buf: Some(tx_buf),
            cs,
        }
    }

    /// Send a single-SPI write with CS toggling, chunking as needed.
    async fn raw_write(&mut self, words: &[u8]) -> Result<(), esp_hal::spi::Error> {
        self.cs.set_low();

        let result = {
            for chunk in words.chunks(DMA_CHUNK) {
                let spi = self.spi.take().unwrap();
                let mut tx_buf = self.tx_buf.take().unwrap();

                tx_buf.as_mut_slice()[..chunk.len()].copy_from_slice(chunk);
                match spi.half_duplex_write(
                    DataMode::Single,
                    Command::None,
                    Address::None,
                    0,
                    chunk.len(),
                    tx_buf,
                ) {
                    Ok(mut transfer) => {
                        transfer.wait_for_done().await;
                        let (sd, tb) = transfer.wait();
                        self.spi = Some(sd);
                        self.tx_buf = Some(tb);
                    }
                    Err((e, sd, tb)) => {
                        self.spi = Some(sd);
                        self.tx_buf = Some(tb);
                        self.cs.set_high();
                        return Err(e);
                    }
                }
            }
            Ok(())
        };

        self.cs.set_high();
        Timer::after_micros(2).await;
        result
    }

    /// Quad-SPI write with command + address header on the first chunk.
    async fn quad_write(
        &mut self,
        flash_cmd: Command,
        flash_addr: Address,
        data: &[u8],
    ) -> Result<(), esp_hal::spi::Error> {
        let mut spi = self.spi.take().unwrap();
        let mut tx_buf = self.tx_buf.take().unwrap();

        self.cs.set_low();

        let mut chunks = data.chunks(DMA_CHUNK);
        let result = {
            // First chunk carries the command + address.
            if let Some(first_chunk) = chunks.next() {
                tx_buf.as_mut_slice()[..first_chunk.len()].copy_from_slice(first_chunk);
                match spi.half_duplex_write(
                    DataMode::Quad,
                    flash_cmd,
                    flash_addr,
                    0,
                    first_chunk.len(),
                    tx_buf,
                ) {
                    Ok(mut transfer) => {
                        transfer.wait_for_done().await;
                        (spi, tx_buf) = transfer.wait();
                    }
                    Err((e, sd, tb)) => {
                        self.cs.set_high();
                        self.spi = Some(sd);
                        self.tx_buf = Some(tb);
                        return Err(e);
                    }
                }
            }

            // Remaining chunks: no command/address, pure data continuation.
            for chunk in chunks {
                tx_buf.as_mut_slice()[..chunk.len()].copy_from_slice(chunk);
                match spi.half_duplex_write(
                    DataMode::Quad,
                    Command::None,
                    Address::None,
                    0,
                    chunk.len(),
                    tx_buf,
                ) {
                    Ok(mut transfer) => {
                        transfer.wait_for_done().await;
                        (spi, tx_buf) = transfer.wait();
                    }
                    Err((e, sd, tb)) => {
                        self.cs.set_high();
                        self.spi = Some(sd);
                        self.tx_buf = Some(tb);
                        return Err(e);
                    }
                }
            }

            Ok(())
        };

        self.cs.set_high();
        Timer::after_micros(2).await;
        self.spi = Some(spi);
        self.tx_buf = Some(tx_buf);
        result
    }
}

impl ErrorType for QspiDisplayBus {
    type Error = esp_hal::spi::Error;
}

impl DisplayBus for QspiDisplayBus {
    async fn write_cmd(&mut self, cmd: &[u8]) -> Result<(), Self::Error> {
        // CO5300 in QSPI mode expects commands wrapped in Flash Page Program (0x02).
        let qspi_cmd = [0x02, 0x00, cmd[0], 0x00];
        self.raw_write(&qspi_cmd).await
    }

    async fn write_cmd_with_params(
        &mut self,
        cmd: &[u8],
        params: &[u8],
    ) -> Result<(), Self::Error> {
        // Wrap in Flash Page Program frame; raw_write handles chunking.
        let mut buf = alloc::vec::Vec::with_capacity(4 + params.len());
        buf.extend_from_slice(&[0x02, 0x00, cmd[0], 0x00]);
        buf.extend_from_slice(params);
        self.raw_write(&buf).await
    }

    async fn write_pixels(
        &mut self,
        cmd: &[u8],
        data: &[u8],
        _metadata: Metadata,
    ) -> Result<(), DisplayError<Self::Error>> {
        // 0x32 = Quad Input Page Program (QSPI 4-line data phase)
        let flash_cmd = Command::_8Bit(0x32, DataMode::Single);
        let addr_val = u32::from(cmd[0]) << 8;
        let flash_addr = Address::_24Bit(addr_val, DataMode::Single);

        self.quad_write(flash_cmd, flash_addr, data)
            .await
            .map_err(DisplayError::BusError)
    }

    fn set_reset(&mut self, _reset: bool) -> Result<(), DisplayError<Self::Error>> {
        Err(DisplayError::Unsupported)
    }
}

/// Full bus: raw QSPI, no TE gating.
pub type ProductionBus = QspiDisplayBus;

/// The CO5300 panel driver for the 466x466 AM151 panel.
pub type ProductionPanel = Co5300<AM151Q466466LK_151_C, Output<'static>, ProductionBus>;

/// Top-level display handle used by the rendering pipeline.
pub type SmartWatchDisplay = DisplayDriver<ProductionBus, ProductionPanel>;

/// Send-safe wrapper for `SmartWatchDisplay`.
///
/// SAFETY: `SmartWatchDisplay` contains `Spi<Async>` which is `!Send` in esp-hal
/// (PhantomData<*const ()>). On single-core ESP32 or when display ops are
/// pinned to one core, this is safe — the peripheral is globally addressable
/// hardware, and the flush task (interrupt executor) is the sole owner.
pub struct SendDisplay(pub SmartWatchDisplay);

// SAFETY: see struct doc comment.
unsafe impl Send for SendDisplay {}

/// Construct and initialise the CO5300 AMOLED display.
pub async fn init_display(
    spi: SpiDma<'static, Async>,
    rx_buf: DmaRxBuf,
    tx_buf: DmaTxBuf,
    cs: Output<'static>,
    rst: Output<'static>,
) -> SmartWatchDisplay {
    info!("display: building bus...");

    let qspi = QspiDisplayBus::new(spi, rx_buf, tx_buf, cs);
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
