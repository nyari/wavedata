pub mod nrzi {
    use crate::{signals::TransitionState, utils::BitVec};

    #[derive(Debug, Clone, Copy)]
    pub enum Error {
        IncorrectTransition,
        IncompleteFrame,
        IncorrectStartOfFrame,
        IncorrectBitStuffingInTransitions,
    }

    #[derive(Debug, Clone)]
    enum NRZIState {
        Begin,
        Bit(usize),
        Done(usize),
    }

    pub struct NRZI {
        stuff_bit_after: usize,
        payload: BitVec,
        frame_offset: usize,
    }

    impl NRZI {
        pub fn parse(frame: &[TransitionState], bit_stuffing: usize) -> Result<Self, Error> {
            let mut result = BitVec::new();
            let mut sm = NRZIState::Begin;

            for (idx, ts) in frame.iter().enumerate() {
                sm = match (sm.clone(), ts) {
                    (NRZIState::Begin, TransitionState::Noise(_)) => Ok(NRZIState::Begin),
                    (NRZIState::Begin, TransitionState::Rising) => Ok(NRZIState::Bit(0)),
                    (NRZIState::Begin, _) => Err(Error::IncorrectStartOfFrame),
                    (NRZIState::Bit(hold_count), TransitionState::Hold(hold_length)) => {
                        if *hold_length + hold_count <= bit_stuffing {
                            for _ in 0..*hold_length {
                                result.push(false);
                            }
                            Ok(NRZIState::Bit(hold_count + hold_length))
                        } else {
                            Err(Error::IncorrectBitStuffingInTransitions)
                        }
                    },
                    (NRZIState::Bit(hold_count), TransitionState::Noise(_)) => {
                        if hold_count >= bit_stuffing {
                            result.truncate_last_incomplete_byte();

                            Ok(NRZIState::Done(idx + 1))
                        } else {
                            Err(Error::IncompleteFrame)
                        }
                    },
                    (NRZIState::Bit(hold_count), _) => {
                        if hold_count < bit_stuffing {
                            result.push(true);
                        }
                        Ok(NRZIState::Bit(0))
                    },
                    (NRZIState::Done(_), _) => break,
                }?;
            }

            if let NRZIState::Done(frame_offset) = sm {
                Ok(Self {
                    stuff_bit_after: bit_stuffing,
                    payload: result,
                    frame_offset,
                })
            } else {
                panic!("Internal error")
            }
        }

        pub fn payload(&self) -> &Vec<u8> {
            self.payload.byte_vec()
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        #[test]
        fn nrzi_test_1() {
            let input = [
                TransitionState::Rising,
                TransitionState::Hold(1),
                TransitionState::Falling,
                TransitionState::Hold(4),
                TransitionState::Rising,
                TransitionState::Hold(1),
                TransitionState::Falling,
                TransitionState::Hold(1),
                TransitionState::Rising,
                TransitionState::Hold(4),
                TransitionState::Falling,
                TransitionState::Rising,
                TransitionState::Hold(2),
                TransitionState::Falling,
                TransitionState::Hold(4),
                TransitionState::Rising,
                TransitionState::Falling,
                TransitionState::Rising,
                TransitionState::Hold(1),
                TransitionState::Falling,
                TransitionState::Hold(3),
                TransitionState::Rising,
                TransitionState::Hold(2),
                TransitionState::Falling,
                TransitionState::Hold(4),
                TransitionState::Noise(1),
            ];

            let result = NRZI::parse(&input, 4).unwrap();

            assert_eq!("ABCD".as_bytes(), result.payload());
            assert_eq!(result.frame_offset, 26);
        }
    }
}
