use defmt::println;
use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::Sender;
use embassy_time::{Duration, Instant, Timer};
use max3010x_async::{AdcRange, SamplingRate};
use max3010x_async::{
    Led, LedPulseWidth, Max3010x, SampleAveraging,
    marker::{ic::Max30102, mode::Oximeter},
};

use crate::drivers::bus::{BusError, I2cPeripheral};
use crate::dsp::{BpmCalculator, FIR_COEFFS, FirFilter, MovingMeanSubtractor, Spo2Calculator};

// Plotter integration
#[cfg(feature = "plot")]
use crate::plotter::PlotMessage;

/// Channel IDs for the plotter (must match registration order)
#[cfg(feature = "plot")]
const CH_RAW_RED: u8 = 0;
#[cfg(feature = "plot")]
const CH_PRE_DC_RED: u8 = 1;
#[cfg(feature = "plot")]
const CH_CLEAN_RED: u8 = 2;
#[cfg(feature = "plot")]
const CH_CLEAN_IR: u8 = 3;

pub struct OxymeterRunner {
    sensor: Max3010x<I2cPeripheral, Max30102, Oximeter>,

    // RED Pipeline
    dc_block_red: MovingMeanSubtractor<100>,
    bandpass_red: FirFilter<201>,

    // IR Pipeline
    dc_block_ir: MovingMeanSubtractor<100>,
    bandpass_ir: FirFilter<201>,

    bpm_calc: BpmCalculator,
    spo2_calc: Spo2Calculator,

    // Plotter for compact text output
    #[cfg(feature = "plot")]
    plot_msg: PlotMessage,
    // #[cfg(feature = "plot")]
    // samples_sent: u32,
}

impl OxymeterRunner {
    pub async fn run(
        mut self,
        sender: Sender<'static, CriticalSectionRawMutex, (u8, u8, u8), 2>,
    ) -> ! {
        // Register all plot channels once at startup
        #[cfg(feature = "plot")]
        self.send_plot_registration();

        loop {
            if self.sensor.get_available_sample_count().await.unwrap_or(0) == 0 {
                Timer::after(Duration::from_millis(10)).await;
                continue;
            }

            let chrono = Instant::now().as_micros();
            let mut read_time = 0u64;
            let mut sample_count = 0u8;

            let mut fifo_buffer = [0u32; 32];

            match self.sensor.read_fifo(&mut fifo_buffer).await {
                Ok(samples_read) => {
                    read_time = Instant::now().as_micros() - chrono;
                    sample_count = samples_read;
                    for i in 0..(samples_read as usize) {
                        let red = fifo_buffer[i * 2];
                        let ir = fifo_buffer[i * 2 + 1];

                        let raw_red = red as f32;
                        let raw_ir = ir as f32;

                        // 1. Strip the DC offset (and save the mean for SpO2 math)
                        let (pre_dc_red, dc_mean_red) = self.dc_block_red.process(raw_red);
                        let (pre_dc_ir, dc_mean_ir) = self.dc_block_ir.process(raw_ir);

                        // 2. Apply the FIR Bandpass
                        let clean_red = self.bandpass_red.process(pre_dc_red);
                        let clean_ir = self.bandpass_ir.process(pre_dc_ir);

                        // 3. Continuously calculate the current SpO2 percentage
                        let current_spo2 = self.spo2_calc.process_sample(
                            clean_red,
                            dc_mean_red,
                            clean_ir,
                            dc_mean_ir,
                        );

                        // Send plot data (only when PLOT feature is enabled)
                        #[cfg(feature = "plot")]
                        self.send_plot_data(raw_red, pre_dc_red, clean_red, clean_ir);

                        // Feed the ultimate smoothed signal into BPM calc
                        if let Some(new_bpm) = self.bpm_calc.process_sample(clean_red) {
                            sender.send((new_bpm as u8, current_spo2 as u8, 36u8));
                        }
                    }
                }
                Err(_) => {
                    // Handle I2C read errors
                }
            }

            let elapsed = Instant::now().as_micros() - chrono;
            println!(
                "DSP: Samples: {=u8}, Loop: {=u64} us, Read: {=u64} us, Process: {=u64} us",
                sample_count,
                elapsed,
                read_time,
                elapsed - read_time
            );

            // Yield to give other I2C tasks (touch) a chance
            // Timer::after(Duration::from_millis(2)).await;
        }
    }

    /// Send channel registration messages to the plotter.
    #[cfg(feature = "plot")]
    fn send_plot_registration(&mut self) {
        let msg = self.plot_msg.register(CH_RAW_RED, "RawRed", (0, 200, 0));
        println!("{}", msg);
        let msg = self
            .plot_msg
            .register(CH_PRE_DC_RED, "PreDC", (200, 200, 0));
        println!("{}", msg);
        let msg = self
            .plot_msg
            .register(CH_CLEAN_RED, "CleanRed", (0, 100, 255));
        println!("{}", msg);
        let msg = self
            .plot_msg
            .register(CH_CLEAN_IR, "CleanIR", (255, 100, 0));
        println!("{}", msg);
    }

    /// Send data frames for all plotted signals.
    #[cfg(feature = "plot")]
    fn send_plot_data(&mut self, raw_red: f32, pre_dc_red: f32, clean_red: f32, clean_ir: f32) {
        // Only send plot data for every 8th sample (~12.5 Hz effective at 100 Hz sensor)
        // to avoid saturating the USB serial port
        // self.samples_sent += 1;
        // if self.samples_sent % 8 != 0 {
        //     return;
        // }

        let msg = self.plot_msg.data(CH_RAW_RED, raw_red);
        println!("{}", msg);
        let msg = self.plot_msg.data(CH_PRE_DC_RED, pre_dc_red);
        println!("{}", msg);
        let msg = self.plot_msg.data(CH_CLEAN_RED, clean_red);
        println!("{}", msg);
        let msg = self.plot_msg.data(CH_CLEAN_IR, clean_ir);
        println!("{}", msg);
    }
}

#[embassy_executor::task]
pub async fn acquisition_task(
    runner: OxymeterRunner,
    sender: Sender<'static, CriticalSectionRawMutex, (u8, u8, u8), 2>,
) {
    runner.run(sender).await;
}

pub struct OxymeterHandle;

impl OxymeterHandle {
    pub async fn start(
        spawner: &Spawner,
        i2c: I2cPeripheral,
        sender: Sender<'static, CriticalSectionRawMutex, (u8, u8, u8), 2>,
    ) -> Result<Self, BusError> {
        let mut sensor = Max3010x::new_max30102(i2c);
        Timer::after(Duration::from_millis(2000)).await;

        sensor.reset().await.unwrap();
        Timer::after(Duration::from_millis(100)).await;

        let mut sensor = sensor.into_oximeter().await.unwrap();

        // 400 Sps with Sa4 = 100 Hz effective sample rate
        sensor
            .set_sampling_rate(SamplingRate::Sps400)
            .await
            .unwrap();
        sensor
            .set_sample_averaging(SampleAveraging::Sa4)
            .await
            .unwrap();
        sensor.set_pulse_amplitude(Led::All, 0x1F).await.unwrap();
        sensor.set_pulse_width(LedPulseWidth::Pw411).await.unwrap();
        sensor.set_adc_range(AdcRange::Fs4k).await.unwrap();
        sensor.enable_fifo_rollover().await.unwrap();
        sensor.clear_fifo().await.unwrap();

        let runner = OxymeterRunner {
            sensor,
            dc_block_red: MovingMeanSubtractor::new(),
            bandpass_red: FirFilter::new(FIR_COEFFS),
            dc_block_ir: MovingMeanSubtractor::new(),
            bandpass_ir: FirFilter::new(FIR_COEFFS),
            bpm_calc: BpmCalculator::new(100.0),
            spo2_calc: Spo2Calculator::new(),
            #[cfg(feature = "plot")]
            plot_msg: PlotMessage::new(),
            // #[cfg(feature = "plot")]
            // samples_sent: 0,
        };

        spawner.spawn(acquisition_task(runner, sender).unwrap());

        Ok(Self {})
    }
}
