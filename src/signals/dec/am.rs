//! # Decode amplitude modulated signals
//!
//! ## Signal description
//!
use std::{collections::VecDeque, ops::Div};

use num::complex::ComplexFloat;

use crate::{
    sampling::{SampleCount, Samples, SamplesMut, SamplingRate},
    signals::{am::Transition, proc::FFT},
    units::{Amplitude, Frequency, Proportion},
    utils::{self, WindowedWeightedAverage},
};

struct BandFilter {
    carrier_frequency: Frequency,
    bandwidth: Frequency,
    sr: SamplingRate,
    fft: FFT,
}

impl BandFilter {
    pub fn new(
        carrier_frequency: Frequency,
        baudrate: Frequency,
        sr: SamplingRate,
        transition_width: Proportion,
    ) -> Self {
        let bandwidth = baudrate / transition_width;
        Self {
            carrier_frequency,
            bandwidth,
            sr,
            fft: FFT::new(),
        }
    }

    pub fn filter(&self, s: SamplesMut) {
        let mut dft = self.fft.fft(Samples(s.0), self.sr);
        dft.filter_band(self.carrier_frequency, self.bandwidth);
        let result = self.fft.fft_inverse(dft.as_mut_slice());
        s.0.iter_mut()
            .zip(result.into_iter())
            .for_each(|(result, idft)| *result = idft.abs())
    }
}

#[cfg(test)]
mod integration_test {
    use num::Zero;

    use crate::{
        encodings::nrzi::Value,
        sampling::{Sampleable, SamplesMut},
        units::Time,
    };

    use super::*;

    struct Params {
        lead_in: Time,
        lead_out: Time,
        carrier_frequency: Frequency,
        sampling_rate: SamplingRate,
        carrier_amplitude: Amplitude,
        baudrate: Frequency,
        transition_width: Proportion,
        high_low: (Amplitude, Amplitude),
        transition_window_divisor: usize,
        stuff_bit: u8,
    }

    impl Params {
        fn total_samples_count_estimate(&self, message_len: usize) -> SampleCount {
            let lead_in_out = self.lead_in + self.lead_out;
            let content = self
                .baudrate
                .cycle_time()
                .mul(8.0)
                .mul(message_len as f32)
                .mul(1.5);
            let total = lead_in_out + content;
            self.sampling_rate * total
        }

        fn lead_in_sample_count(&self) -> SampleCount {
            self.sampling_rate * self.lead_in
        }
    }

    fn create_signal_with_message(message: &str, p: &Params) -> (Vec<f32>, Vec<Transition>) {
        let mut result = Vec::with_capacity(p.total_samples_count_estimate(message.len()).value());
        result.resize(p.total_samples_count_estimate(message.len()).value(), 0.0);

        let carrier_signal = crate::sampling::WaveSampler::new(crate::waves::Sine::new(
            p.carrier_frequency,
            Time::zero(),
            p.carrier_amplitude,
        ));

        let nrzi_params = crate::encodings::enc::nrzi::Parameters::new(
            message.as_bytes().iter().map(|x| x.clone()).collect(),
            p.stuff_bit,
            0,
        );

        let data_signal = crate::sampling::SignalSampler::new(crate::signals::enc::am::NRZI::new(
            crate::signals::enc::am::NRZIConsts::new(p.baudrate, p.transition_width, p.high_low),
            nrzi_params.clone(),
        ));

        let mut composite_sampler =
            crate::sampling::CompositeSampler::new(carrier_signal, data_signal, |input, output| {
                *output = input.0 * input.1;
            });

        composite_sampler.sample_into_f32(
            SamplesMut(&mut result[p.lead_in_sample_count().value()..]),
            p.sampling_rate,
        );

        let transitions = {
            let values: Vec<Value> = crate::encodings::enc::nrzi::NRZI::new(nrzi_params).collect();
            crate::signals::enc::am::utils::nrzi_to_transition_states(&values, p.stuff_bit as usize)
                .unwrap()
        };

        (result, transitions)
    }
}
