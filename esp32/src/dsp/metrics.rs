/// Calculates the rolling mean of squares (Energy) for RMS calculation
pub struct RollingEnergy<const N: usize> {
    buffer: [f32; N],
    index: usize,
    count: usize,
}

impl<const N: usize> RollingEnergy<N> {
    pub const fn new() -> Self {
        Self {
            buffer: [0.0; N],
            index: 0,
            count: 0,
        }
    }

    pub fn process(&mut self, ac_value: f32) -> f32 {
        self.buffer[self.index] = ac_value * ac_value;
        self.index = (self.index + 1) % N;
        if self.count < N {
            self.count += 1;
        }

        let mut sum_squares = 0.0;
        for i in 0..self.count {
            sum_squares += self.buffer[i];
        }

        sum_squares / (self.count as f32)
    }
}

/// Detects peaks in a filtered PPG signal using a dynamic decaying threshold
/// and a refractory period to reject the dicrotic notch.
pub struct BpmCalculator {
    sample_rate: f32,
    samples_since_last_beat: u32,
    last_sample: f32,
    dynamic_threshold: f32,
    decay_rate: f32,
    min_threshold: f32,
    refractory_samples: u32,
}

impl BpmCalculator {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            samples_since_last_beat: 0,
            last_sample: 0.0,
            dynamic_threshold: 0.0,
            decay_rate: 0.99,
            min_threshold: 10.0,
            refractory_samples: (sample_rate * 0.3) as u32,
        }
    }

    pub fn process_sample(&mut self, current_sample: f32) -> Option<f32> {
        self.samples_since_last_beat += 1;
        let mut detected_bpm = None;

        self.dynamic_threshold *= self.decay_rate;
        if self.dynamic_threshold < self.min_threshold {
            self.dynamic_threshold = self.min_threshold;
        }

        if self.samples_since_last_beat > self.refractory_samples {
            if self.last_sample > self.dynamic_threshold && current_sample < self.last_sample {
                self.dynamic_threshold = self.last_sample * 0.8;

                let time_in_seconds = self.samples_since_last_beat as f32 / self.sample_rate;
                let bpm = 60.0 / time_in_seconds;

                if bpm > 40.0 && bpm < 210.0 {
                    detected_bpm = Some(bpm);
                }

                self.samples_since_last_beat = 0;
            }
        }

        self.last_sample = current_sample;
        detected_bpm
    }
}

/// Calculates SpO2 using the Ratio of Ratios method
pub struct Spo2Calculator {
    energy_red: RollingEnergy<100>,
    energy_ir: RollingEnergy<100>,
}

impl Spo2Calculator {
    pub const fn new() -> Self {
        Self {
            energy_red: RollingEnergy::new(),
            energy_ir: RollingEnergy::new(),
        }
    }

    pub fn process_sample(&mut self, ac_red: f32, dc_red: f32, ac_ir: f32, dc_ir: f32) -> f32 {
        let mean_sq_red = self.energy_red.process(ac_red);
        let mean_sq_ir = self.energy_ir.process(ac_ir);

        if dc_red == 0.0 || dc_ir == 0.0 || mean_sq_ir == 0.0 {
            return 98.0;
        }

        let rms_red = libm::sqrtf(mean_sq_red);
        let rms_ir = libm::sqrtf(mean_sq_ir);

        let ratio_red = rms_red / dc_red;
        let ratio_ir = rms_ir / dc_ir;

        let r = ratio_red / ratio_ir;

        let mut spo2 = 104.0 - 17.0 * r;

        if spo2 > 100.0 {
            spo2 = 100.0;
        }
        if spo2 < 50.0 {
            spo2 = 50.0;
        }

        spo2
    }
}
