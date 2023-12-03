use num::Zero;

use crate::encodings::nrzi::Value;
use crate::encodings::{self};
use crate::units::{Amplitude, Frequency, Proportion, Time};

use crate::signals::{BinaryLevel, Signal};

type NRZIEncoder = encodings::enc::nrzi::NRZI;

pub struct NRZIConsts {
    transition_width: Time,
    baud_length: Time,
    highlow: (Amplitude, Amplitude),
}

impl NRZIConsts {
    pub fn new(
        baudrate: Frequency,
        transition_width: Proportion,
        highlow: (Amplitude, Amplitude),
    ) -> Self {
        Self {
            baud_length: baudrate.cycle_time(),
            transition_width: baudrate.cycle_time() * transition_width,
            highlow: highlow,
        }
    }
}

struct NRZIState {
    nrzi: NRZIEncoder,
    current_transition_progress: Time,
    current_level: BinaryLevel,
}

impl NRZIState {
    pub fn init(nrzi_params: encodings::enc::nrzi::Parameters) -> Self {
        Self {
            nrzi: NRZIEncoder::new(nrzi_params),
            current_transition_progress: Time::zero(),
            current_level: BinaryLevel::Low,
        }
    }
}

pub struct NRZI {
    c: NRZIConsts,
    m: NRZIState,
}

impl NRZI {
    pub fn new(c: NRZIConsts, nrzi_params: encodings::enc::nrzi::Parameters) -> Self {
        Self {
            c: c,
            m: NRZIState::init(nrzi_params),
        }
    }

    fn level_to_amplitude(&self, level: BinaryLevel) -> Amplitude {
        match level {
            BinaryLevel::Low => self.c.highlow.1,
            BinaryLevel::High => self.c.highlow.0,
        }
    }

    fn current_value(&self) -> Amplitude {
        if !self.transition() {
            self.level_to_amplitude(self.m.current_level)
        } else {
            self.caluclate_transition_slope()
        }
    }

    fn caluclate_transition_slope(&self) -> Amplitude {
        let progress =
            (self.m.current_transition_progress / self.c.transition_width).clamp(0.0, 1.0);
        let (from, to) = (
            self.level_to_amplitude(self.m.current_level),
            self.level_to_amplitude(self.m.current_level.neg()),
        );
        let delta = to - from;
        from + (delta.scale(progress))
    }

    fn advance(&mut self, dt: Time) -> Result<(), crate::signals::Error> {
        if dt > self.c.transition_width {
            return Err(crate::signals::Error::Undersampled);
        }

        self.m.current_transition_progress += dt;
        if self.m.current_transition_progress >= self.c.baud_length {
            self.m.current_transition_progress -= self.c.baud_length;
            if self.transition() {
                self.m.current_level = self.m.current_level.neg()
            }
            self.m.nrzi.advance();
        }

        if let Value::Complete = self.m.nrzi.current() {
            Err(crate::signals::Error::Finished)
        } else {
            Ok(())
        }
    }

    fn transition(&self) -> bool {
        match self.m.nrzi.current() {
            Value::StartOfFrame | Value::StuffBit | Value::Bit(true) => true,
            Value::EndOfFrame(eofidx) => match (self.m.current_level, eofidx) {
                (BinaryLevel::Low, 0) => true,
                (BinaryLevel::Low, _) => false,
                (BinaryLevel::High, 0) => true,
                (BinaryLevel::High, 1) => true,
                (BinaryLevel::High, _) => false,
            },
            _ => false,
        }
    }
}

impl Signal for NRZI {
    fn advance_with(&mut self, dt: Time) -> Result<Amplitude, crate::signals::Error> {
        let result = self.current_value();
        self.advance(dt)?;
        Ok(result)
    }
}

pub mod utils {
    use crate::{
        encodings::nrzi::Value,
        signals::{BinaryLevel, TransitionState},
    };

    pub fn nrzi_to_transition_states(input: &[Value]) -> Result<Vec<TransitionState>, ()> {
        let mut result = Vec::new();
        let mut level = BinaryLevel::Low;
        for value in input {
            level = match (level, value) {
                (BinaryLevel::Low, Value::StartOfFrame) => {
                    result.push(TransitionState::Rising);
                    Ok(BinaryLevel::High)
                },
                (level, Value::StuffBit) | (level, Value::Bit(true)) => {
                    result.push(level.transition());
                    Ok(level.neg())
                },
                (level, Value::Bit(false)) => {
                    result.push(TransitionState::Hold(1));
                    Ok(level)
                },
                (BinaryLevel::Low, Value::EndOfFrame(0)) => {
                    result.push(TransitionState::Rising);
                    Ok(BinaryLevel::High)
                },
                (BinaryLevel::High, Value::EndOfFrame(eof)) if *eof <= 1 => {
                    result.push(TransitionState::Falling);
                    Ok(BinaryLevel::Low)
                },
                (BinaryLevel::Low, Value::EndOfFrame(_)) => {
                    result.push(TransitionState::Hold(1));
                    Ok(BinaryLevel::Low)
                },
                (BinaryLevel::Low, Value::Complete) => {
                    result.push(TransitionState::Noise(1));
                    break;
                },
                _ => Err(()),
            }?;
        }

        Ok(result.into_iter().fold(Vec::new(), |mut acc, item| {
            if !acc.is_empty() {
                let action = match (acc.last().unwrap(), item) {
                    (TransitionState::Hold(prev), TransitionState::Hold(curr)) => {
                        Some(TransitionState::Hold(prev + curr))
                    },
                    (TransitionState::Noise(prev), TransitionState::Noise(curr)) => {
                        Some(TransitionState::Noise(prev + curr))
                    },
                    _ => None,
                };

                match action {
                    Some(update) => *acc.last_mut().unwrap() = update,
                    None => acc.push(item),
                }

                acc
            } else {
                acc.push(item);
                acc
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nrzi_full_test_ending_zero_1() {
        let mut nrzi = NRZI::new(
            NRZIConsts::new(
                Frequency::new(1.0),
                Proportion::new(1.0),
                (Amplitude::new(1.0), Amplitude::new(0.0)),
            ),
            encodings::enc::nrzi::Parameters::new(vec![0b_0100_0010_u8], 4),
        );

        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        );
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.5)
        );
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(1.0)
        ); // End of start of frame
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(1.0)
        ); // Mid 1. bit
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(1.0)
        ); // End 1. bit
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.5)
        ); // Mid 2. bit
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // End 2. bit
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // Mid 3. bit
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // End 3. bit
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // Mid 4. bit
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // End 4. bit
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // Mid 5. bit
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // End 5. bit
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // Mid 6. bit
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // End 6. bit
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.5)
        ); // Mid stuff bit
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(1.0)
        ); // End stuff bit
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.5)
        ); // Mid 7. bit
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // End 7. bit
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // Mid 8. bit
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // End 8. bit
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.5)
        ); // Mid EOF 0
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(1.0)
        ); // End EOF 0
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.5)
        ); // Mid EOF 1
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // End EOF 1
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // Mid EOF 2
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // End EOF 2
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // Mid EOF 3
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // End EOF 3
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // Mid EOF 4
        assert_eq!(
            nrzi.advance_with(Time::new(0.5)).unwrap(),
            Amplitude::new(0.0)
        ); // End EOF 4
        assert!(matches!(
            nrzi.advance_with(Time::new(0.5)),
            Err(crate::signals::Error::Finished)
        ));
    }
}
