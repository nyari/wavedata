pub mod units;
pub mod waves;

pub mod signals {
    use crate::units::{Time, Amplitude, Frequency};
    pub enum Error {
        Undersampled
    }

    pub trait Signal : Sized + Send {
        fn advance_with(&mut self, dt: Time) -> Result<Amplitude, Error>;
    }

    pub struct Proportion(f32);

    /// Amplitude modulated signals
    pub mod am {
        use crate::units::{Time, Amplitude, Frequency};

        use super::Proportion;

        pub struct SqareWaveDataNRZConsts {
            baudrate: Frequency,
            transition_width: Proportion,
            stuff_bit_width: u8,
            payload: Vec<u8>, // Bytes
            baud_length: f32
        }

        struct SqareWaveDataNRZState {
            payload_offset: usize,
            current_bit_offset: u8,
            contigous_zeros: u8,
            current_transition_progress: f32
        }

        impl SqareWaveDataNRZState {

        }

        pub struct SqareWaveDataNRZ {
            c: SqareWaveDataNRZConsts,
            m: SqareWaveDataNRZState
        }
    }
}

pub mod sampling {
    use crate::units::{Amplitude, Time, Frequency};

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
}


#[cfg(test)]
mod tests {
    use super::*;

}
