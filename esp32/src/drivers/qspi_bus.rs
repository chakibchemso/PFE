//! Minimal SPI+CS bridge to [`display_driver::bus::DisplayBus`].
//!
//! The CO5300 operates similarly to a SPI Flash chip when QSPI is enabled (SPI_MODE = 0x80).
//! It requires commands to be wrapped in SPI Flash Page Program (`0x02`) or
//! Quad Page Program (`0x32`) formats.
//! This implementation intercepts those and sends them natively using `esp-hal`'s
//! DMA-based async half-duplex transfers, utilising all 4 data lines for
//! high-bandwidth pixel data.

use display_driver::DisplayError;
use display_driver::bus::{DisplayBus, ErrorType, Metadata};
use esp_hal::Async;
use esp_hal::dma::DmaTxBuf;
use esp_hal::gpio::Output;
use esp_hal::spi::master::{Address, Command, DataMode, SpiDma, SpiDmaBus};

/// DMA transfer size — matches the board-level DMA buffer capacity.
const DMA_CHUNK: usize = 32736 / 4; // 8192

/// Thin wrapper: raw SPI bus + CS pin, implementing the QSPI Flash-style display interface.
///
/// Uses `Option<SpiDmaBus>` so `SpiDisplayBus` can temporarily take ownership of the bus,
/// split it into `(SpiDma, rx_buf, tx_buf)`, and use `SpiDma::half_duplex_write` + async
/// `SpiDmaTransfer::wait_for_done` for non-blocking QSPI pixel transfers (interrupt-driven
/// rather than spin-wait).
pub struct SpiDisplayBus {
    bus: Option<SpiDmaBus<'static, Async>>,
    cs: Output<'static>,
}

impl SpiDisplayBus {
    pub fn new(bus: SpiDmaBus<'static, Async>, cs: Output<'static>) -> Self {
        Self { bus: Some(bus), cs }
    }

    /// Async QSPI half-duplex write via `SpiDmaTransfer` (interrupt-driven, not spin-wait).
    ///
    /// Takes the bus out of `Option`, splits into `SpiDma + buffers`, runs the transfer,
    /// and puts everything back before returning (even on error).
    async fn quad_write(
        bus: &mut Option<SpiDmaBus<'static, Async>>,
        cs: &mut Output<'static>,
        flash_cmd: Command,
        flash_addr: Address,
        data: &[u8],
    ) -> Result<(), esp_hal::spi::Error> {
        let spi_bus = bus
            .take()
            .expect("SpiDisplayBus: bus already taken in quad_write");
        let (mut spi_dma, rx_buf, mut tx_buf) = spi_bus.split();

        cs.set_low();

        let result =
            Self::run_quad_write(&mut spi_dma, &mut tx_buf, flash_cmd, flash_addr, data).await;

        cs.set_high();
        embassy_time::Timer::after_micros(2).await;
        *bus = Some(SpiDmaBus::new(spi_dma, rx_buf, tx_buf));
        result
    }

    /// Inner quad write — moves `spi_dma`/`tx_buf` into
    /// `SpiDma::half_duplex_write` + `SpiDmaTransfer` for each chunk.
    ///
    /// On error the parts are stashed back via `*spi_dma` / `*tx_buf` so the
    /// caller can reconstruct the bus.
    #[allow(clippy::too_many_arguments)]
    async fn run_quad_write(
        spi_dma: &mut SpiDma<'static, Async>,
        tx_buf: &mut DmaTxBuf,
        flash_cmd: Command,
        flash_addr: Address,
        data: &[u8],
    ) -> Result<(), esp_hal::spi::Error> {
        let mut chunks = data.chunks(DMA_CHUNK);
        if let Some(chunk) = chunks.next() {
            // We need to take ownership temporarily — use ptr::read/write pairs.
            // Safety: each `read` is mirrored by a `write` before any path
            // reaches code that observes the slot again.
            //
            // For the error path, the tuple `(SpiDma, DmaTxBuf)` from the Err
            // variant is written back into the slots.
            let owned_sd;
            let mut owned_tb;
            // SAFETY: round-trip through raw pointers.
            unsafe {
                owned_sd = core::ptr::read(spi_dma);
                owned_tb = core::ptr::read(tx_buf);
            }

            owned_tb.as_mut_slice()[..chunk.len()].copy_from_slice(chunk);

            match owned_sd.half_duplex_write(
                DataMode::Quad,
                flash_cmd,
                flash_addr,
                0,
                chunk.len(),
                owned_tb,
            ) {
                Ok(mut transfer) => {
                    transfer.wait_for_done().await;
                    let (sd, tb) = transfer.wait();
                    // SAFETY: write back into the slots.
                    unsafe {
                        core::ptr::write(spi_dma, sd);
                        core::ptr::write(tx_buf, tb);
                    }
                }
                Err((e, sd, tb)) => {
                    unsafe {
                        core::ptr::write(spi_dma, sd);
                        core::ptr::write(tx_buf, tb);
                    }
                    return Err(e);
                }
            }
        }

        for chunk in chunks {
            let owned_sd;
            let mut owned_tb;
            unsafe {
                owned_sd = core::ptr::read(spi_dma);
                owned_tb = core::ptr::read(tx_buf);
            }

            owned_tb.as_mut_slice()[..chunk.len()].copy_from_slice(chunk);

            match owned_sd.half_duplex_write(
                DataMode::Quad,
                Command::None,
                Address::None,
                0,
                chunk.len(),
                owned_tb,
            ) {
                Ok(mut transfer) => {
                    transfer.wait_for_done().await;
                    let (sd, tb) = transfer.wait();
                    unsafe {
                        core::ptr::write(spi_dma, sd);
                        core::ptr::write(tx_buf, tb);
                    }
                }
                Err((e, sd, tb)) => {
                    unsafe {
                        core::ptr::write(spi_dma, sd);
                        core::ptr::write(tx_buf, tb);
                    }
                    return Err(e);
                }
            }
        }

        Ok(())
    }
}

impl ErrorType for SpiDisplayBus {
    type Error = esp_hal::spi::Error;
}

impl DisplayBus for SpiDisplayBus {
    async fn write_cmd(&mut self, cmd: &[u8]) -> Result<(), Self::Error> {
        self.cs.set_low();
        let qspi_cmd = [0x02, 0x00, cmd[0], 0x00];
        self.bus
            .as_mut()
            .expect("SpiDisplayBus: bus not available in write_cmd")
            .write_async(&qspi_cmd)
            .await?;
        self.cs.set_high();
        embassy_time::Timer::after_micros(2).await;
        Ok(())
    }

    async fn write_cmd_with_params(
        &mut self,
        cmd: &[u8],
        params: &[u8],
    ) -> Result<(), Self::Error> {
        self.cs.set_low();

        let mut buf = alloc::vec::Vec::with_capacity(4 + params.len());
        buf.extend_from_slice(&[0x02, 0x00, cmd[0], 0x00]);
        buf.extend_from_slice(params);

        for chunk in buf.chunks(DMA_CHUNK) {
            self.bus
                .as_mut()
                .expect("SpiDisplayBus: bus not available in write_cmd_with_params")
                .write_async(chunk)
                .await?;
        }

        self.cs.set_high();
        embassy_time::Timer::after_micros(2).await;
        Ok(())
    }

    async fn write_pixels(
        &mut self,
        cmd: &[u8],
        data: &[u8],
        _metadata: Metadata,
    ) -> Result<(), DisplayError<Self::Error>> {
        // 0x32 = Quad Input Page Program
        let flash_cmd = Command::_8Bit(0x32, DataMode::Single);
        // Address is 0x00_CMD_00
        let addr_val = (cmd[0] as u32) << 8;
        let flash_addr = Address::_24Bit(addr_val, DataMode::Single);

        Self::quad_write(&mut self.bus, &mut self.cs, flash_cmd, flash_addr, data)
            .await
            .map_err(DisplayError::BusError)
    }

    fn set_reset(&mut self, _reset: bool) -> Result<(), DisplayError<Self::Error>> {
        Err(DisplayError::Unsupported)
    }
}
