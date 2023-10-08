use crate::units::Time;

/// Number of samplings per second
pub struct SamplingRate(usize);
impl SamplingRate {
    pub fn new(value: usize) -> Self { Self(value) }
    pub fn value(self) -> usize { self.0 }
}
/// Number of samples taken
pub struct Samples(usize);

impl From<usize> for Samples {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl SamplingRate {
    fn sample(&self, amount: Samples) -> Time {
        let rate = self.0 as f32;
        let amount = amount.0 as f32;
        Time::new(amount / rate)
    }

    fn increment(&self) -> Time {
        Time::new(1.0f32 / (self.0 as f32))
    }
}

pub trait Sampleable: Send {
    fn sample_into_f32(&self, out: &mut [f32], rate: SamplingRate) -> Time;
}

impl<T: crate::waves::Wave> Sampleable for T {
    fn sample_into_f32(&self, out: &mut [f32], rate: SamplingRate) -> Time
    {
        let length = rate.sample(Samples::from(out.len()));
        let increment = rate.increment();
        
        for (sample_idx, sample_value) in out.iter_mut().enumerate() {
            let amplitude = self.value_at(increment * (sample_idx as f32));
            *sample_value = amplitude.value();
        }

        length
    }
}