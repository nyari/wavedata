use crate::{
    sampling::{Samples, SamplingRate},
    units::{Frequency, Proportion, Time},
};

pub struct Parameters {
    carrier_frequency: Frequency,
    transition_width: Time,
    baud_length: Time,
}

impl Parameters {
    pub fn new(
        carrier_frequency: Frequency,
        baudrate: Frequency,
        transition_width_proportion: Proportion,
    ) -> Self {
        let baud_length = baudrate.cycle_time();
        Self {
            carrier_frequency: carrier_frequency,
            baud_length: baud_length,
            transition_width: baud_length * transition_width_proportion.value(),
        }
    }
}
struct TransitionDecoder {
    c: Parameters,
}

impl TransitionDecoder {
    pub fn new(c: Parameters) -> Self {
        Self { c: c }
    }

    pub fn decode_sample(samples: Samples, rate: SamplingRate) -> (Vec<bool>, Time) {
        todo!()
    }
}
