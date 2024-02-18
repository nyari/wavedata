pub mod nrzi {
    use crate::encodings::nrzi::Value;

    #[derive(Clone, Copy)]
    enum StateMachine {
        BeginOfFrame,
        Payload,
        EndOfFrame,
        Complete,
    }

    #[derive(Clone)]
    pub struct Parameters {
        payload: Vec<u8>, // Bytes
        stuff_bit_after: u8,
        preamble: u8,
    }

    impl Parameters {
        pub fn new(payload: Vec<u8>, stuff_bit_after: u8, preamble: u8) -> Self {
            Self {
                payload,
                stuff_bit_after,
                preamble,
            }
        }
    }

    struct State {
        payload_offset: usize,
        current_bit_offset: u8,
        contigous_zeros: u8,
        preamble: u8,
        sm: StateMachine,
    }

    impl State {
        pub fn init() -> Self {
            Self {
                payload_offset: 0,
                current_bit_offset: 0,
                contigous_zeros: 0,
                sm: StateMachine::BeginOfFrame,
                preamble: 0,
            }
        }
    }

    pub struct NRZI {
        c: Parameters,
        m: State,
    }

    impl NRZI {
        pub fn new(c: Parameters) -> Self {
            Self {
                c: c,
                m: State::init(),
            }
        }

        pub fn current(&self) -> Value {
            match self.m.sm {
                StateMachine::BeginOfFrame => Value::StartOfFrame(self.m.preamble),
                StateMachine::Payload => {
                    if !self.stuffing() {
                        Value::Bit(self.current_bit())
                    } else {
                        Value::StuffBit
                    }
                },
                StateMachine::EndOfFrame => Value::EndOfFrame(self.m.contigous_zeros),
                StateMachine::Complete => Value::Complete,
            }
        }

        pub fn advance(&mut self) {
            let sm = self.m.sm.clone();
            self.m.sm = match sm {
                StateMachine::BeginOfFrame => {
                    self.m.preamble += 1;
                    if self.m.preamble < self.c.preamble {
                        StateMachine::BeginOfFrame
                    } else {
                        StateMachine::Payload
                    }
                },
                StateMachine::Payload => {
                    let last_bit = self.current_bit();
                    if !self.stuffing() {
                        if !last_bit {
                            self.m.contigous_zeros += 1;
                        } else {
                            self.m.contigous_zeros = 0;
                        }
                        self.advance_bit();
                    } else {
                        self.m.contigous_zeros = 0;
                    }

                    if !self.is_end_of_frame() {
                        StateMachine::Payload
                    } else {
                        self.m.contigous_zeros = 0;
                        StateMachine::EndOfFrame
                    }
                },
                StateMachine::EndOfFrame => {
                    self.m.contigous_zeros += 1;
                    if self.m.contigous_zeros > self.c.stuff_bit_after + 1 {
                        StateMachine::Complete
                    } else {
                        StateMachine::EndOfFrame
                    }
                },
                StateMachine::Complete => StateMachine::Complete,
            }
        }

        fn advance_bit(&mut self) {
            if self.m.current_bit_offset < 7 {
                self.m.current_bit_offset += 1
            } else {
                self.m.current_bit_offset = 0;
                self.m.payload_offset += 1;
            }
        }

        fn current_bit(&self) -> bool {
            let byte = self.c.payload[self.m.payload_offset];
            let mask_byte = 0b1_u8 << (7 - self.m.current_bit_offset);
            byte & mask_byte != 0
        }

        fn stuffing(&self) -> bool {
            self.m.contigous_zeros >= self.c.stuff_bit_after
        }

        fn is_end_of_frame(&self) -> bool {
            self.m.payload_offset >= self.c.payload.len()
        }
    }

    impl Iterator for NRZI {
        type Item = Value;

        fn next(&mut self) -> Option<Self::Item> {
            let result = self.current();
            self.advance();
            match result {
                Value::Complete => None,
                value => Some(value),
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn null_byte_without_bit_stuffing() {
            let nrzi = NRZI::new(Parameters {
                payload: vec![0b_0000_0000],
                stuff_bit_after: 9,
                preamble: 8,
            });
            assert_eq!(
                nrzi.collect::<Vec<Value>>(),
                vec![
                    Value::StartOfFrame(0),
                    Value::StartOfFrame(1),
                    Value::StartOfFrame(2),
                    Value::StartOfFrame(3),
                    Value::StartOfFrame(4),
                    Value::StartOfFrame(5),
                    Value::StartOfFrame(6),
                    Value::StartOfFrame(7),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::EndOfFrame(0),
                    Value::EndOfFrame(1),
                    Value::EndOfFrame(2),
                    Value::EndOfFrame(3),
                    Value::EndOfFrame(4),
                    Value::EndOfFrame(5),
                    Value::EndOfFrame(6),
                    Value::EndOfFrame(7),
                    Value::EndOfFrame(8),
                    Value::EndOfFrame(9),
                    Value::EndOfFrame(10)
                ]
            );
        }
        #[test]
        fn null_byte_with_symmetric_bit_stuffing() {
            let nrzi = NRZI::new(Parameters {
                payload: vec![0b_0000_0000],
                stuff_bit_after: 4,
                preamble: 4,
            });
            assert_eq!(
                nrzi.collect::<Vec<Value>>(),
                vec![
                    Value::StartOfFrame(0),
                    Value::StartOfFrame(1),
                    Value::StartOfFrame(2),
                    Value::StartOfFrame(3),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::StuffBit,
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::EndOfFrame(0),
                    Value::EndOfFrame(1),
                    Value::EndOfFrame(2),
                    Value::EndOfFrame(3),
                    Value::EndOfFrame(4),
                    Value::EndOfFrame(5)
                ]
            );
        }

        #[test]
        fn null_byte_with_assymetric_bit_stuffing() {
            let nrzi = NRZI::new(Parameters {
                payload: vec![0b_0000_0000],
                stuff_bit_after: 5,
                preamble: 0,
            });
            assert_eq!(
                nrzi.collect::<Vec<Value>>(),
                vec![
                    Value::StartOfFrame(0),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::StuffBit,
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::EndOfFrame(0),
                    Value::EndOfFrame(1),
                    Value::EndOfFrame(2),
                    Value::EndOfFrame(3),
                    Value::EndOfFrame(4),
                    Value::EndOfFrame(5),
                    Value::EndOfFrame(6)
                ]
            );
        }

        #[test]
        fn byte_without_bit_stuffing_needed() {
            let nrzi = NRZI::new(Parameters {
                payload: vec![0b_1001_1000],
                stuff_bit_after: 4,
                preamble: 0,
            });
            assert_eq!(
                nrzi.collect::<Vec<Value>>(),
                vec![
                    Value::StartOfFrame(0),
                    Value::Bit(true),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(true),
                    Value::Bit(true),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::EndOfFrame(0),
                    Value::EndOfFrame(1),
                    Value::EndOfFrame(2),
                    Value::EndOfFrame(3),
                    Value::EndOfFrame(4),
                    Value::EndOfFrame(5)
                ]
            );
        }

        #[test]
        fn byte_with_bit_stuffing_needed() {
            let nrzi = NRZI::new(Parameters {
                payload: vec![0b_1000_0100],
                stuff_bit_after: 4,
                preamble: 0,
            });
            assert_eq!(
                nrzi.collect::<Vec<Value>>(),
                vec![
                    Value::StartOfFrame(0),
                    Value::Bit(true),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::StuffBit,
                    Value::Bit(true),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::EndOfFrame(0),
                    Value::EndOfFrame(1),
                    Value::EndOfFrame(2),
                    Value::EndOfFrame(3),
                    Value::EndOfFrame(4),
                    Value::EndOfFrame(5)
                ]
            );
        }

        #[test]
        fn multibyte_test_1() {
            let nrzi = NRZI::new(Parameters {
                payload: vec![0b_1001_1000, 0b_0010_0010],
                stuff_bit_after: 4,
                preamble: 0,
            });
            assert_eq!(
                nrzi.collect::<Vec<Value>>(),
                vec![
                    Value::StartOfFrame(0),
                    Value::Bit(true),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(true),
                    Value::Bit(true),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::StuffBit,
                    Value::Bit(false),
                    Value::Bit(true),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(false),
                    Value::Bit(true),
                    Value::Bit(false),
                    Value::EndOfFrame(0),
                    Value::EndOfFrame(1),
                    Value::EndOfFrame(2),
                    Value::EndOfFrame(3),
                    Value::EndOfFrame(4),
                    Value::EndOfFrame(5)
                ]
            );
        }
    }
}
