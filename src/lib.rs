pub mod waves {
    /// Time offset of a signal in terms of seconds
    #[derive(Clone, Copy)]
    pub struct Time(f32);

    impl Time {
        pub fn new(value: f32) -> Self { Self(value) }
        pub fn zero() -> Self { Self (0f32) }
        pub fn value(self) -> f32 { self.0 }
    }

    impl std::ops::Add for Time {
        type Output = Time;

        fn add(self, rhs: Self) -> Self::Output {
            Self(self.0 + rhs.0)
        }
    }

    impl<T> std::ops::Mul<T> for Time
        where T: std::ops::Mul<f32, Output=f32>
    {
        type Output = Time;

        fn mul(self, rhs: T) -> Self::Output {
            Time(rhs * self.0)
        }

    }
    /// Frequency of a signal in terms of 1/s a.k.a Hz
    #[derive(Clone, Copy)]
    pub struct Frequency(f32);

    impl Frequency {
        pub fn new(value: f32) -> Self { Self(value) }
        pub fn value(self) -> f32 { self.0 }
    }
    /// Maximum amplitude of a signal
    #[derive(Clone, Copy)]
    pub struct Amplitude(f32);

    impl Amplitude {
        pub fn new(value: f32) -> Self { Self(value) }
        pub fn value(self) -> f32 { self.0 }
    }

    pub trait Wave : Sized + Send {
        fn shift_mut(&mut self, offset: Time);
        fn value_at(&self, t: Time) -> Amplitude;


        fn shift(mut self, offset: Time) -> Self {
            self.shift_mut(offset);
            self
        }
    }

    pub struct Sine {
        freq: Frequency,
        phase_offset: Time,
        amplitude: Amplitude
    }

    impl Sine {
        pub fn new(freq: Frequency, phase_offset: Time, amplitude: Amplitude) -> Self {
            Self {
                freq: freq,
                phase_offset: phase_offset,
                amplitude: amplitude
            }
        }
    }

    impl Wave for Sine {
        fn shift_mut(&mut self, offset: Time) {
            let new_phase_offset_base = self.phase_offset + offset;
            let cycle_time = self.freq.value().recip();
            let whole_phases = (new_phase_offset_base.value() / cycle_time).floor();
            self.phase_offset = Time::new(new_phase_offset_base.value() - (whole_phases * cycle_time));
        }

        fn value_at(&self, t: Time) -> Amplitude {
            let offset_t = self.phase_offset.value() + t.value();
            let apply_pi = offset_t * 2.0f32 * std::f32::consts::PI;
            let apply_frequency = apply_pi * self.freq.value();
            Amplitude::new(apply_frequency.sin() * self.amplitude.value())
        }
    }
}

pub mod sampling {
    use crate::waves;

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
        fn sample(&self, amount: Samples) -> waves::Time {
            let rate = self.0 as f32;
            let amount = amount.0 as f32;
            waves::Time::new(amount / rate)
        }

        fn increment(&self) -> waves::Time {
            waves::Time::new(1.0f32 / (self.0 as f32))
        }
    }
    
    pub trait Sampleable: Send {
        fn sample_into_f32(&self, out: &mut [f32], rate: SamplingRate) -> waves::Time;
    }

    impl<T: crate::waves::Wave> Sampleable for T {
        fn sample_into_f32(&self, out: &mut [f32], rate: SamplingRate) -> waves::Time
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
}


#[cfg(test)]
mod tests {
    use super::*;

}
