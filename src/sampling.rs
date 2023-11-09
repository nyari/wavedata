use crate::units::{Amplitude, Frequency, Proportion, Time};

pub struct Samples<'a>(pub &'a [f32]);

impl<'a> Samples<'a> {
    pub fn count(&self) -> SampleCount {
        SampleCount(self.0.len())
    }
}
pub struct SamplesMut<'a>(pub &'a mut [f32]);

/// Number of samples taken
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SampleCount(usize);

impl SampleCount {
    pub fn new(samples: usize) -> Self {
        Self(samples)
    }

    pub fn value(self) -> usize {
        self.0
    }
}

impl std::ops::Div<SamplingRate> for SampleCount {
    type Output = Time;
    fn div(self, rhs: SamplingRate) -> Self::Output {
        Time::new((self.0 / rhs.0) as f32)
    }
}

impl std::ops::Div<Time> for SampleCount {
    type Output = SamplingRate;
    fn div(self, rhs: Time) -> Self::Output {
        SamplingRate::new((self.0 as f32 / rhs.value()) as usize)
    }
}

impl std::ops::Div<usize> for SampleCount {
    type Output = SampleCount;
    fn div(self, rhs: usize) -> Self::Output {
        SampleCount(self.0 / rhs)
    }
}

impl std::ops::Mul<usize> for SampleCount {
    type Output = SampleCount;
    fn mul(self, rhs: usize) -> Self::Output {
        SampleCount(self.0 * rhs)
    }
}

impl std::ops::Mul<Proportion> for SampleCount {
    type Output = Self;
    fn mul(self, rhs: Proportion) -> Self::Output {
        let result_value = self.0 as f32 * rhs.value();
        SampleCount(result_value as usize)
    }
}

impl From<usize> for SampleCount {
    fn from(value: usize) -> Self {
        Self(value)
    }
}
/// Number of samplings per second
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Eq, Ord)]
pub struct SamplingRate(usize);
impl SamplingRate {
    pub fn new(value: usize) -> Self {
        Self(value)
    }
    pub fn value(self) -> usize {
        self.0
    }
    pub fn max_frequency(self) -> Frequency {
        Frequency::new((self.0 / 2) as f32)
    }
}

impl SamplingRate {
    fn sample(&self, amount: SampleCount) -> Time {
        let rate = self.0 as f32;
        let amount = amount.0 as f32;
        Time::new(amount / rate)
    }

    fn increment(&self) -> Time {
        Time::new(1.0f32 / (self.0 as f32))
    }
}

impl std::ops::Mul<Time> for SamplingRate {
    type Output = SampleCount;
    fn mul(self, rhs: Time) -> Self::Output {
        SampleCount::new((self.0 as f32 * rhs.value()).ceil() as usize)
    }
}

pub trait Sampleable: Send {
    fn sample_into_f32(&mut self, out: SamplesMut, rate: SamplingRate);
}

pub struct WaveSampler<T>(T);

impl<T: Sized> WaveSampler<T> {
    pub fn new(t: T) -> Self {
        Self(t)
    }
}

impl<T: crate::waves::Wave> Sampleable for WaveSampler<T> {
    fn sample_into_f32(&mut self, out: SamplesMut, rate: SamplingRate) {
        let length = rate.sample(SampleCount::from(out.0.len()));
        let increment = rate.increment();

        for (sample_idx, sample_value) in out.0.iter_mut().enumerate() {
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
    fn sample_into_f32(&mut self, out: SamplesMut, rate: SamplingRate) {
        let increment = rate.increment();

        for sample_value in out.0.iter_mut() {
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
    fn sample_into_f32(&mut self, out: SamplesMut, rate: SamplingRate) {
        if out.0.len() != self.buffer.0.len() {
            self.buffer.0.resize(out.0.len(), 0.0);
            self.buffer.1.resize(out.0.len(), 0.0);
        }

        self.s
            .0
            .sample_into_f32(SamplesMut(self.buffer.0.as_mut_slice()), rate);
        self.s
            .1
            .sample_into_f32(SamplesMut(self.buffer.1.as_mut_slice()), rate);

        out.0
            .iter_mut()
            .zip(self.buffer.0.iter().zip(self.buffer.1.iter()))
            .for_each(|(out, s)| (self.compositor)(s, out));
    }
}
