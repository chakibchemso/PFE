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
        self.history[self.index] = input;

        let mut out = 0.0;
        let mut history_idx = self.index;

        for coeff_idx in 0..N {
            out += self.coeffs[coeff_idx] * self.history[history_idx];
            history_idx = if history_idx == 0 {
                N - 1
            } else {
                history_idx - 1
            };
        }

        self.index = (self.index + 1) % N;
        out
    }
}

/// Removes the baseline and returns BOTH the AC signal and the DC Mean
pub struct MovingMeanSubtractor<const N: usize> {
    buffer: [f32; N],
    index: usize,
    count: usize,
}

impl<const N: usize> MovingMeanSubtractor<N> {
    pub const fn new() -> Self {
        Self {
            buffer: [0.0; N],
            index: 0,
            count: 0,
        }
    }

    /// Returns (ac_value, dc_mean)
    pub fn process(&mut self, new_value: f32) -> (f32, f32) {
        self.buffer[self.index] = new_value;
        self.index = (self.index + 1) % N;

        if self.count < N {
            self.count += 1;
        }

        let mut exact_sum = 0.0;
        for i in 0..self.count {
            exact_sum += self.buffer[i];
        }

        let dc_mean = exact_sum / (self.count as f32);
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
        self.sum -= self.buffer[self.index];
        self.buffer[self.index] = new_value;
        self.sum += new_value;
        self.index = (self.index + 1) % N;
        self.sum / (N as f32)
    }
}
