use crate::units::{Amplitude, Time};

pub mod dec;
/// Amplitude modulated signals
pub mod enc;
pub mod filters;

#[derive(Debug)]
pub enum Error {
    Undersampled,
    Finished,
}

pub trait Signal: Sized + Send {
    fn advance_with(&mut self, dt: Time) -> Result<Amplitude, Error>;
}

#[derive(Clone, Copy)]
enum BinaryLevel {
    Low,
    High,
}

impl BinaryLevel {
    fn neg(self) -> Self {
        match self {
            Self::High => Self::Low,
            Self::Low => Self::High,
        }
    }
}

pub struct CompositeSignal<F, S1, S2>
where
    F: Fn((Amplitude, Amplitude), Time) -> Amplitude + Send,
    S1: Signal,
    S2: Signal,
{
    s: (S1, S2),
    compositor: F,
}

impl<F, S1, S2> CompositeSignal<F, S1, S2>
where
    F: Fn((Amplitude, Amplitude), Time) -> Amplitude + Send,
    S1: Signal,
    S2: Signal,
{
    pub fn new(s1: S1, s2: S2, compositor: F) -> Self {
        Self {
            s: (s1, s2),
            compositor: compositor,
        }
    }
}

impl<F, S1, S2> Signal for CompositeSignal<F, S1, S2>
where
    F: Fn((Amplitude, Amplitude), Time) -> Amplitude + Send,
    S1: Signal,
    S2: Signal,
{
    fn advance_with(&mut self, dt: Time) -> Result<Amplitude, Error> {
        let a = (self.s.0.advance_with(dt)?, self.s.1.advance_with(dt)?);
        Ok((self.compositor)(a, dt))
    }
}
