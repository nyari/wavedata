use crate::units::{Time, Amplitude};

/// Amplitude modulated signals
pub mod am;


#[derive(Debug)]
pub enum Error {
    Undersampled,
    Finished
}

pub trait Signal : Sized + Send {
    fn advance_with(&mut self, dt: Time) -> Result<Amplitude, Error>;
}

#[derive(Clone, Copy)]
enum BinaryLevel {
    Low,
    High
}

impl BinaryLevel {
    fn neg(self) -> Self {
        match self {
            Self::High => Self::Low,
            Self::Low => Self::High
        }
    }
}

