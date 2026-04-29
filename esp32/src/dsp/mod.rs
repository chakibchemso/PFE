pub mod coeffs;
pub mod filters;
pub mod metrics;

pub use coeffs::FIR_COEFFS;
pub use filters::{FirFilter, MovingAverage, MovingMeanSubtractor};
pub use metrics::{BpmCalculator, RollingEnergy, Spo2Calculator};
