/// Removes the baseline and returns BOTH the AC signal and the DC Mean
pub struct MovingMeanSubtractor<const N: usize> {
    buffer: [f32; N],
    index: usize,
    sum: f32,
    count: usize,
}

impl<const N: usize> MovingMeanSubtractor<N> {
    pub const fn new() -> Self {
        Self {
            buffer: [0.0; N],
            index: 0,
            sum: 0.0,
            count: 0,
        }
    }

    /// Returns (ac_value, dc_mean)
    pub fn process(&mut self, new_value: f32) -> (f32, f32) {
        self.sum -= self.buffer[self.index];
        self.buffer[self.index] = new_value;
        self.sum += new_value;
        self.index = (self.index + 1) % N;

        if self.count < N {
            self.count += 1;
        }

        let dc_mean = self.sum / (self.count as f32);
        let ac_value = new_value - dc_mean;

        (ac_value, dc_mean)
    }
}

/// A simple ring-buffer based moving average filter
pub struct MovingAverage<const N: usize> {
    buffer: [f32; N],
    index: usize,
    sum: f32,
}

impl<const N: usize> MovingAverage<N> {
    pub fn new() -> Self {
        Self {
            buffer: [0.0; N],
            index: 0,
            sum: 0.0,
        }
    }

    pub fn process(&mut self, new_value: f32) -> f32 {
        // Subtract the oldest value from the sum
        self.sum -= self.buffer[self.index];

        // Add the new value to the buffer and the sum
        self.buffer[self.index] = new_value;
        self.sum += new_value;

        // Advance the index, wrapping around
        self.index = (self.index + 1) % N;

        // Return the average
        self.sum / (N as f32)
    }
}

/// Calculates the rolling mean of squares (Energy) for RMS calculation
pub struct RollingEnergy<const N: usize> {
    buffer: [f32; N],
    index: usize,
    sum_squares: f32,
    count: usize,
}

impl<const N: usize> RollingEnergy<N> {
    pub const fn new() -> Self {
        Self {
            buffer: [0.0; N],
            index: 0,
            sum_squares: 0.0,
            count: 0,
        }
    }

    pub fn process(&mut self, ac_value: f32) -> f32 {
        let squared = ac_value * ac_value;

        self.sum_squares -= self.buffer[self.index];
        self.buffer[self.index] = squared;
        self.sum_squares += squared;

        self.index = (self.index + 1) % N;
        if self.count < N {
            self.count += 1;
        }

        // Return the mean of squares
        self.sum_squares / (self.count as f32)
    }
}

/// Detects peaks in a filtered PPG signal using a dynamic decaying threshold
/// and a refractory period to reject the dicrotic notch.
pub struct BpmCalculator {
    sample_rate: f32,
    samples_since_last_beat: u32,
    last_sample: f32,

    // Dynamic peak tracking
    dynamic_threshold: f32,
    decay_rate: f32,
    min_threshold: f32,

    // Time-based rejection
    refractory_samples: u32,
}

impl BpmCalculator {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            samples_since_last_beat: 0,
            last_sample: 0.0,

            dynamic_threshold: 0.0,
            // Decays the threshold by 1% every sample (adjust based on sample rate)
            decay_rate: 0.99,
            // The absolute minimum AC amplitude to be considered a pulse
            min_threshold: 10.0,

            // 300ms refractory period. At 100Hz, this is 30 samples.
            // This caps the max detectable heart rate at 200 BPM (60s / 0.3s)
            refractory_samples: (sample_rate * 0.3) as u32,
        }
    }

    /// Feeds a mathematically smoothed AC sample into the calculator.
    pub fn process_sample(&mut self, current_sample: f32) -> Option<f32> {
        self.samples_since_last_beat += 1;
        let mut detected_bpm = None;

        // 1. Decay the dynamic threshold so it doesn't get stuck on one massive peak
        self.dynamic_threshold *= self.decay_rate;
        if self.dynamic_threshold < self.min_threshold {
            self.dynamic_threshold = self.min_threshold;
        }

        // 2. Are we outside the refractory period? (Ignoring the dicrotic notch)
        if self.samples_since_last_beat > self.refractory_samples {
            // 3. Peak Detection: Did the signal just cross the threshold and start going down?
            if self.last_sample > self.dynamic_threshold && current_sample < self.last_sample {
                // We found a valid heartbeat peak!

                // Snap the threshold up to 80% of this new peak's height
                self.dynamic_threshold = self.last_sample * 0.8;

                // Calculate BPM based on the time since the last peak
                let time_in_seconds = self.samples_since_last_beat as f32 / self.sample_rate;
                let bpm = 60.0 / time_in_seconds;

                // Final sanity check for extreme physiological outliers
                if bpm > 40.0 && bpm < 210.0 {
                    detected_bpm = Some(bpm);
                }

                // Reset the timer for the next beat
                self.samples_since_last_beat = 0;
            }
        }

        self.last_sample = current_sample;
        detected_bpm
    }
}

/// Calculates SpO2 using the Ratio of Ratios method
pub struct Spo2Calculator {
    energy_red: RollingEnergy<100>, // 1-second rolling window at 100Hz
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

        // Prevent division by zero during sensor startup
        if dc_red == 0.0 || dc_ir == 0.0 || mean_sq_ir == 0.0 {
            return 98.0; // Safe default
        }

        // Calculate RMS using libm for no_std compatibility
        let rms_red = libm::sqrtf(mean_sq_red);
        let rms_ir = libm::sqrtf(mean_sq_ir);

        let ratio_red = rms_red / dc_red;
        let ratio_ir = rms_ir / dc_ir;

        let r = ratio_red / ratio_ir;

        // Maxim's standard linear approximation formula
        let mut spo2 = 104.0 - 17.0 * r;

        // Clamp to realistic physiological bounds
        if spo2 > 100.0 {
            spo2 = 100.0;
        }
        if spo2 < 50.0 {
            spo2 = 50.0;
        }

        spo2
    }
}

/// A Finite Impulse Response (FIR) filter.
pub struct FirFilter<const N: usize> {
    coeffs: [f32; N],
    history: [f32; N],
    index: usize,
}

impl<const N: usize> FirFilter<N> {
    pub const fn new(coeffs: [f32; N]) -> Self {
        Self {
            coeffs,
            history: [0.0; N],
            index: 0,
        }
    }

    pub fn process(&mut self, input: f32) -> f32 {
        // 1. Store the newest sample
        self.history[self.index] = input;

        // 2. Compute the convolution (multiply-accumulate)
        let mut out = 0.0;
        let mut history_idx = self.index;

        for coeff_idx in 0..N {
            out += self.coeffs[coeff_idx] * self.history[history_idx];

            // Wrap backwards through the circular buffer
            history_idx = if history_idx == 0 {
                N - 1
            } else {
                history_idx - 1
            };
        }

        // 3. Advance the index
        self.index = (self.index + 1) % N;

        out
    }
}

// use: tools/fir-coef-gen.py
pub const FIR_COEFFS: [f32; 201] = [
    -7.166252423601985e-19f32,
    4.810361606267235e-05f32,
    9.52641704697624e-05f32,
    0.00013948687460272683f32,
    0.00017866110339591874f32,
    0.00021056128086710722f32,
    0.00023287335282958017f32,
    0.00024325262129699105f32,
    0.00023941563955346074f32,
    0.00021926521642916548f32,
    0.0001810436419076852f32,
    0.00012350535180807564f32,
    4.609674045471448e-05f32,
    -5.087195963905159e-05f32,
    -0.00016607958568260736f32,
    -0.0002970891153012953f32,
    -0.00044030138753197774f32,
    -0.0005909816083744557f32,
    -0.0007433690067061594f32,
    -0.0008908749837080766f32,
    -0.0010263690886491538f32,
    -0.0011425455154885156f32,
    -0.001232355981022026f32,
    -0.0012894882962154774f32,
    -0.001308864174570763f32,
    -0.0012871253147183402f32,
    -0.0012230739778425846f32,
    -0.00111803349939492f32,
    -0.0009760956617852768f32,
    -0.0008042257078858383f32,
    -0.0006122019409449475f32,
    -0.00041237512444516395f32,
    -0.00021924290146201473f32,
    -4.884569313244579e-05f32,
    8.199761066120606e-05f32,
    0.00015658409325309592f32,
    0.0001593103899477795f32,
    7.66950786958024e-05f32,
    -0.00010161747552703153f32,
    -0.00038193330763259083f32,
    -0.0007657273014751876f32,
    -0.0012490631964928896f32,
    -0.0018222677332753298f32,
    -0.002469901150158958f32,
    -0.003171052568882914f32,
    -0.0038999723979081893f32,
    -0.004627035504779897f32,
    -0.0053200094956517095f32,
    -0.005945583041379378f32,
    -0.006471090920210256f32,
    -0.006866356424899505f32,
    -0.007105559063660432f32,
    -0.007169027001097162f32,
    -0.00704485018355265f32,
    -0.006730212082436547f32,
    -0.006232345700305246f32,
    -0.005569032843785078f32,
    -0.004768584284571057f32,
    -0.0038692616042307892f32,
    -0.0029181282635137524f32,
    -0.0019693465197449292f32,
    -0.0010819668145078544f32,
    -0.0003172856253213431f32,
    0.00026412506585830805f32,
    0.000605590131540004f32,
    0.0006575607000379334f32,
    0.0003807439688602233f32,
    -0.0002510277324581313f32,
    -0.0012483464235385586f32,
    -0.002604342818980443f32,
    -0.004293492491878047f32,
    -0.0062712141705231796f32,
    -0.008474327599233186f32,
    -0.01082240064492984f32,
    -0.013219974399183647f32,
    -0.015559612816206013f32,
    -0.01772568197830226f32,
    -0.019598725461149408f32,
    -0.021060268478658065f32,
    -0.02199785634114861f32,
    -0.0223101138070168f32,
    -0.02191160233299121f32,
    -0.020737252788290073f32,
    -0.018746162172117938f32,
    -0.015924564036947586f32,
    -0.012287812942741594f32,
    -0.007881262141642973f32,
    -0.002779959181850691f32,
    0.002912865769571927f32,
    0.009068491609434572f32,
    0.0155364982850796f32,
    0.02214952946223934f32,
    0.02872885121135937f32,
    0.03509049406980348f32,
    0.041051736790280256f32,
    0.04643767144235964f32,
    0.051087582286822954f32,
    0.05486087545326689f32,
    0.05764231282283455f32,
    0.059346330995019216f32,
    0.059920263614981546f32,
    0.059346330995019216f32,
    0.05764231282283455f32,
    0.05486087545326689f32,
    0.051087582286822954f32,
    0.04643767144235964f32,
    0.041051736790280256f32,
    0.03509049406980348f32,
    0.028728851211359367f32,
    0.02214952946223934f32,
    0.015536498285079596f32,
    0.009068491609434572f32,
    0.0029128657695719267f32,
    -0.002779959181850691f32,
    -0.007881262141642972f32,
    -0.01228781294274159f32,
    -0.015924564036947586f32,
    -0.018746162172117935f32,
    -0.02073725278829007f32,
    -0.021911602332991203f32,
    -0.022310113807016797f32,
    -0.021997856341148602f32,
    -0.021060268478658054f32,
    -0.019598725461149404f32,
    -0.017725681978302255f32,
    -0.015559612816206008f32,
    -0.013219974399183647f32,
    -0.010822400644929836f32,
    -0.008474327599233184f32,
    -0.0062712141705231796f32,
    -0.004293492491878047f32,
    -0.0026043428189804416f32,
    -0.0012483464235385582f32,
    -0.0002510277324581313f32,
    0.00038074396886022323f32,
    0.0006575607000379334f32,
    0.0006055901315400036f32,
    0.00026412506585830794f32,
    -0.000317285625321343f32,
    -0.0010819668145078541f32,
    -0.001969346519744929f32,
    -0.00291812826351375f32,
    -0.003869261604230788f32,
    -0.004768584284571055f32,
    -0.005569032843785077f32,
    -0.0062323457003052455f32,
    -0.006730212082436542f32,
    -0.007044850183552645f32,
    -0.0071690270010971585f32,
    -0.007105559063660431f32,
    -0.006866356424899504f32,
    -0.0064710909202102505f32,
    -0.0059455830413793756f32,
    -0.005320009495651707f32,
    -0.0046270355047798965f32,
    -0.0038999723979081884f32,
    -0.0031710525688829114f32,
    -0.0024699011501589565f32,
    -0.001822267733275329f32,
    -0.0012490631964928894f32,
    -0.0007657273014751876f32,
    -0.0003819333076325905f32,
    -0.00010161747552703145f32,
    7.669507869580235e-05f32,
    0.0001593103899477795f32,
    0.0001565840932530957f32,
    8.199761066120596e-05f32,
    -4.884569313244573e-05f32,
    -0.00021924290146201463f32,
    -0.00041237512444516395f32,
    -0.0006122019409449468f32,
    -0.0008042257078858372f32,
    -0.0009760956617852757f32,
    -0.0011180334993949193f32,
    -0.0012230739778425846f32,
    -0.0012871253147183387f32,
    -0.0013088641745707614f32,
    -0.0012894882962154767f32,
    -0.0012323559810220254f32,
    -0.0011425455154885156f32,
    -0.0010263690886491525f32,
    -0.0008908749837080758f32,
    -0.0007433690067061589f32,
    -0.0005909816083744552f32,
    -0.00044030138753197774f32,
    -0.0002970891153012948f32,
    -0.00016607958568260706f32,
    -5.087195963905154e-05f32,
    4.609674045471446e-05f32,
    0.00012350535180807564f32,
    0.000181043641907685f32,
    0.0002192652164291651f32,
    0.00023941563955346058f32,
    0.00024325262129699091f32,
    0.00023287335282958017f32,
    0.00021056128086710695f32,
    0.00017866110339591863f32,
    0.00013948687460272675f32,
    9.52641704697624e-05f32,
    4.810361606267235e-05f32,
    -7.166252423601985e-19f32,
];
