use crate::units::{Amplitude, Time};

/// Number of samplings per second
#[derive(Clone, Copy)]
pub struct SamplingRate(usize);
impl SamplingRate {
    pub fn new(value: usize) -> Self {
        Self(value)
    }
    pub fn value(self) -> usize {
        self.0
    }
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
    fn sample_into_f32(&mut self, out: &mut [f32], rate: SamplingRate);
}

pub struct WaveSampler<T>(T);

impl<T: Sized> WaveSampler<T> {
    pub fn new(t: T) -> Self {
        Self(t)
    }
}

impl<T: crate::waves::Wave> Sampleable for WaveSampler<T> {
    fn sample_into_f32(&mut self, out: &mut [f32], rate: SamplingRate) {
        let length = rate.sample(Samples::from(out.len()));
        let increment = rate.increment();

        for (sample_idx, sample_value) in out.iter_mut().enumerate() {
            let amplitude = self.0.value_at(increment * (sample_idx as f32));
            *sample_value = amplitude.value();
        }

        self.0.shift_mut(length);
    }
}

pub struct SignalSampler<T>(T);

impl<T: Sized> SignalSampler<T> {
    pub fn new(t: T) -> Self {
        Self(t)
    }
}

impl<T: crate::signals::Signal> Sampleable for SignalSampler<T> {
    fn sample_into_f32(&mut self, out: &mut [f32], rate: SamplingRate) {
        let increment = rate.increment();

        for sample_value in out.iter_mut() {
            let amplitude = match self.0.advance_with(increment) {
                Ok(amplitude) => amplitude,
                Err(crate::signals::Error::Finished) => Amplitude::zero(),
                _ => panic!("Unhandleable error during sampling"),
            };
            *sample_value = amplitude.value();
        }
    }
}

pub struct CompositeSampler<F, S1, S2>
where
    F: Fn((&f32, &f32), &mut f32) -> () + Send,
    S1: Sampleable,
    S2: Sampleable,
{
    compositor: F,
    s: (S1, S2),
    buffer: (Vec<f32>, Vec<f32>),
}

impl<F, S1, S2> CompositeSampler<F, S1, S2>
where
    F: Fn((&f32, &f32), &mut f32) -> () + Send,
    S1: Sampleable,
    S2: Sampleable,
{
    pub fn new(s1: S1, s2: S2, compositor: F) -> Self {
        Self {
            compositor: compositor,
            s: (s1, s2),
            buffer: (Vec::new(), Vec::new()),
        }
    }
}

impl<F, S1, S2> Sampleable for CompositeSampler<F, S1, S2>
where
    F: Fn((&f32, &f32), &mut f32) -> () + Send,
    S1: Sampleable,
    S2: Sampleable,
{
    fn sample_into_f32(&mut self, out: &mut [f32], rate: SamplingRate) {
        if out.len() != self.buffer.0.len() {
            self.buffer.0.resize(out.len(), 0.0);
            self.buffer.1.resize(out.len(), 0.0);
        }

        self.s.0.sample_into_f32(self.buffer.0.as_mut_slice(), rate);
        self.s.1.sample_into_f32(self.buffer.1.as_mut_slice(), rate);

        out.iter_mut()
            .zip(self.buffer.0.iter().zip(self.buffer.1.iter()))
            .for_each(|(out, s)| (self.compositor)(s, out));
    }
}
