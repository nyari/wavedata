use crate::units::{Time, Amplitude, Frequency};
pub enum Error {
    Undersampled,
    Finished
}

pub trait Signal : Sized + Send {
    fn advance_with(&mut self, dt: Time) -> Result<Amplitude, Error>;
}

pub struct Proportion(f32);

/// Amplitude modulated signals
pub mod am {
    enum Level {
        Low,
        High
    }

    use crate::units::{Time, Amplitude, Frequency};

    use super::{Proportion, Signal};

    pub struct NRZConsts {
        baudrate: Frequency,
        transition_width: Proportion,
        stuff_bit_after: u8,
        payload: Vec<u8>, // Bytes
        baud_length: Time,
        highlow: (Amplitude, Amplitude)
    }

    impl NRZConsts {
        pub fn new(baudrate: Frequency, transition_width: Proportion, stuff_bit_after: u8, payload: Vec<u8>, highlow: (Amplitude, Amplitude)) -> Self {
            Self {
                baudrate: baudrate,
                transition_width: transition_width,
                stuff_bit_after: stuff_bit_after,
                payload: payload,
                baud_length: baudrate.cycle_time(),
                highlow: highlow
            }
        }
    }

    struct NRZState {
        payload_offset: usize,
        current_bit_offset: u8,
        contigous_zeros: u8,
        current_transition_progress: f32,
        current_level: Level
    }

    impl NRZState {
        pub fn init() -> Self {
            Self {
                payload_offset: 0,
                current_bit_offset: 0,
                contigous_zeros: 0,
                current_transition_progress: 0.0,
                current_level: Level::Low,
            }
        }
    }

    pub struct NRZ {
        c: NRZConsts,
        m: NRZState
    }

    impl NRZ {
        pub fn new(c: NRZConsts) -> Self {
            Self {
                c: c,
                m: NRZState::init()
            }
        }

        fn current_value(&self) -> Amplitude {
            let bit = self.current_bit();

            todo!()
        }

        fn advance_with(&mut self, dt: Time) -> Result<Amplitude, super::Error> {
            todo!()
        }

        fn current_bit(&self) -> bool {
            let byte = self.c.payload[self.m.payload_offset];
            let mask_byte = 0b1_u8 << self.m.current_bit_offset;
            byte & mask_byte != 0
        }

        fn stuffing(&self) -> bool {
            self.m.contigous_zeros >= self.c.stuff_bit_after
        }

    }

    impl Signal for NRZ {
        fn advance_with(&mut self, dt: Time) -> Result<Amplitude, super::Error>
        {
            let result = self.current_value();
            self.advance_with(dt)?;
            Ok(result)
        }
    }
}