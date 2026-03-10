use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::watch::{Receiver, Watch};
use embassy_time::{Duration, Timer};
use esp_hal::{
    Blocking,
    i2c::master::{Error, I2c},
};
use max3010x::{AdcRange, SamplingRate};
use max3010x::{
    Led, LedPulseWidth, Max3010x, SampleAveraging,
    marker::{ic::Max30102, mode::Oximeter},
};

// Brought back your custom DSP structs!
use crate::processor::{
    BpmCalculator, FIR_COEFFS, FirFilter, MovingMeanSubtractor, Spo2Calculator,
};

/// BPM, SpO2, Temp
static DATA_CHANNEL: Watch<CriticalSectionRawMutex, (f32, f32, f32), 1> = Watch::new();

pub struct OxymeterRunner {
    sensor: Max3010x<I2c<'static, Blocking>, Max30102, Oximeter>,

    // RED Pipeline
    dc_blocker_red: MovingMeanSubtractor<100>,
    bandpass_red: FirFilter<201>,

    // IR Pipeline
    dc_blocker_ir: MovingMeanSubtractor<100>,
    bandpass_ir: FirFilter<201>,

    bpm_calc: BpmCalculator,
    spo2_calc: Spo2Calculator,
}

impl OxymeterRunner {
    pub async fn run(mut self) -> ! {
        let sender = DATA_CHANNEL.sender();

        loop {
            if self.sensor.get_available_sample_count().unwrap_or(0) == 0 {
                Timer::after(Duration::from_millis(1)).await;
                continue;
            }

            let mut fifo_buffer = [0u32; 32];

            match self.sensor.read_fifo(&mut fifo_buffer) {
                Ok(samples_read) => {
                    for i in 0..(samples_read as usize) {
                        let red = fifo_buffer[i * 2];
                        let ir = fifo_buffer[i * 2 + 1];

                        let raw_red = red as f32;
                        let raw_ir = ir as f32;

                        // 1. Strip the DC offset (and save the mean for SpO2 math)
                        let (pre_dc_red, dc_mean_red) = self.dc_blocker_red.process(raw_red);
                        let (pre_dc_ir, dc_mean_ir) = self.dc_blocker_ir.process(raw_ir);

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

                        // ---------------------------------

                        // DUMP TO SERIAL FOR PYTHON
                        defmt::println!(
                            "Raw,{=f32},PreDC,{=f32},CleanRed,{=f32},CleanIr,{=f32}",
                            raw_red,
                            pre_dc_red,
                            clean_red,
                            clean_ir
                        );

                        // Feed the ultimate smoothed signal into BPM calc
                        if let Some(new_bpm) = self.bpm_calc.process_sample(clean_red) {
                            sender.send((new_bpm, current_spo2, 36.0));
                        }
                    }
                }
                Err(_) => {
                    // Handle I2C read errors
                }
            }
        }
    }
}

#[embassy_executor::task]
pub async fn acquisition_task(runner: OxymeterRunner) {
    runner.run().await;
}

pub struct OxymeterHandle {
    receiver: Receiver<'static, CriticalSectionRawMutex, (f32, f32, f32), 1>,
}

impl OxymeterHandle {
    pub async fn start(spawner: &Spawner, i2c: I2c<'static, Blocking>) -> Result<Self, Error> {
        let mut sensor = Max3010x::new_max30102(i2c);

        sensor.reset().unwrap();
        Timer::after(Duration::from_millis(100)).await;

        let mut sensor = sensor.into_oximeter().unwrap();

        // 400 SPS with Sa4 = 100 Hz effective sample rate
        sensor.set_sampling_rate(SamplingRate::Sps400).unwrap();
        sensor.set_pulse_width(LedPulseWidth::Pw411).unwrap();
        sensor.set_adc_range(AdcRange::Fs4k).unwrap();
        sensor.set_pulse_amplitude(Led::All, 0x1F).unwrap();
        sensor.set_sample_averaging(SampleAveraging::Sa4).unwrap();
        sensor.enable_fifo_rollover().unwrap();
        sensor.clear_fifo().unwrap();

        let runner = OxymeterRunner {
            sensor,
            dc_blocker_red: MovingMeanSubtractor::new(),
            bandpass_red: FirFilter::new(FIR_COEFFS),
            dc_blocker_ir: MovingMeanSubtractor::new(),
            bandpass_ir: FirFilter::new(FIR_COEFFS),
            bpm_calc: BpmCalculator::new(100.0),
            spo2_calc: Spo2Calculator::new(),
        };

        let receiver = DATA_CHANNEL
            .receiver()
            .expect("Failed to get Watch receiver");

        spawner
            .spawn(acquisition_task(runner))
            .expect("Task queue full");

        Ok(Self { receiver })
    }

    fn get_latest_data(&mut self) -> (f32, f32, f32) {
        self.receiver.try_get().unwrap_or_default()
    }

    pub fn bpm(&mut self) -> f32 {
        self.get_latest_data().0
    }

    pub fn spo2(&mut self) -> f32 {
        self.get_latest_data().1
    }

    pub fn temp(&mut self) -> f32 {
        self.get_latest_data().2
    }
}
