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