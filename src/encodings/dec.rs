pub mod nrzi {
    use crate::{signals::am::Transition, utils::BitVec};

    #[derive(Debug, Clone, Copy)]
    pub enum Error {
        IncorrectTransition,
        IncompleteFrame,
        IncorrectStartOfFrame,
        IncorrectBitStuffingInTransitions,
    }

    #[derive(Debug, Clone)]
    enum NRZIState {
        Begin(u8),
        Bit(usize),
        Done(usize),
    }

    pub struct NRZI {
        stuff_bit_after: usize,
        payload: BitVec,
        frame_offset: usize,
        preamble: u8,
    }

    impl NRZI {
        pub fn parse(
            frame: &[Transition],
            bit_stuffing: usize,
            preamble: u8,
        ) -> Result<Self, Error> {
            let mut result = BitVec::new();
            let mut sm = NRZIState::Begin(0);
            let mut sof = 0u8;

            for (idx, ts) in frame.iter().enumerate() {
                sm = match (sm.clone(), ts) {
                    (NRZIState::Begin(0), Transition::Noise(_)) => Ok(NRZIState::Begin(0)),
                    (NRZIState::Begin(value), Transition::Rising) if value % 2 == 0 => {
                        sof += 1;
                        if sof < preamble {
                            Ok(NRZIState::Begin(sof))
                        } else {
                            Ok(NRZIState::Bit(0))
                        }
                    },
                    (NRZIState::Begin(value), Transition::Falling) if value % 2 == 1 => {
                        sof += 1;
                        if sof < preamble {
                            Ok(NRZIState::Begin(sof))
                        } else {
                            Ok(NRZIState::Bit(0))
                        }
                    },
                    (NRZIState::Begin(_), _) => Err(Error::IncorrectStartOfFrame),
                    (NRZIState::Bit(hold_count), Transition::Hold(hold_length)) => {
                        if *hold_length + hold_count <= bit_stuffing {
                            for _ in 0..*hold_length {
                                result.push(false);
                            }
                            Ok(NRZIState::Bit(hold_count + hold_length))
                        } else {
                            Err(Error::IncorrectBitStuffingInTransitions)
                        }
                    },
                    (NRZIState::Bit(hold_count), Transition::Noise(_)) => {
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
                    preamble,
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
                Transition::Rising,
                Transition::Hold(1),
                Transition::Falling,
                Transition::Hold(4),
                Transition::Rising,
                Transition::Hold(1),
                Transition::Falling,
                Transition::Hold(1),
                Transition::Rising,
                Transition::Hold(4),
                Transition::Falling,
                Transition::Rising,
                Transition::Hold(2),
                Transition::Falling,
                Transition::Hold(4),
                Transition::Rising,
                Transition::Falling,
                Transition::Rising,
                Transition::Hold(1),
                Transition::Falling,
                Transition::Hold(3),
                Transition::Rising,
                Transition::Hold(2),
                Transition::Falling,
                Transition::Hold(4),
                Transition::Noise(1),
            ];

            let result = NRZI::parse(&input, 4, 0).unwrap();

            assert_eq!("ABCD".as_bytes(), result.payload());
            assert_eq!(result.frame_offset, input.len());
        }
        #[test]
        fn nrzi_test_2() {
            let input = [
                Transition::Rising,
                Transition::Falling,
                Transition::Rising,
                Transition::Falling,
                Transition::Rising,
                Transition::Hold(1),
                Transition::Falling,
                Transition::Hold(4),
                Transition::Rising,
                Transition::Hold(1),
                Transition::Falling,
                Transition::Hold(1),
                Transition::Rising,
                Transition::Hold(4),
                Transition::Falling,
                Transition::Rising,
                Transition::Hold(2),
                Transition::Falling,
                Transition::Hold(4),
                Transition::Rising,
                Transition::Falling,
                Transition::Rising,
                Transition::Hold(1),
                Transition::Falling,
                Transition::Hold(3),
                Transition::Rising,
                Transition::Hold(2),
                Transition::Falling,
                Transition::Hold(4),
                Transition::Noise(1),
            ];

            let result = NRZI::parse(&input, 4, 5).unwrap();

            assert_eq!("ABCD".as_bytes(), result.payload());
            assert_eq!(result.frame_offset, input.len());
        }
    }
}
