//! # Decode amplitude modulated signals
//!
//! ## Signal description
//!
use std::{cell::RefCell, collections::VecDeque, ops::Div};

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
        dft.filter_band(self.carrier_frequency, self.bandwidth)
            .unwrap();
        let result = self.fft.fft_inverse(dft.as_mut_slice());
        s.0.iter_mut()
            .zip(result.into_iter())
            .for_each(|(result, idft)| *result = idft.abs())
    }
}

struct EnvelopeCalculation {
    buffer: Vec<f32>,
}

impl EnvelopeCalculation {
    pub fn new(carrier_wave_cycle: SampleCount) -> Self {
        Self {
            buffer: {
                let mut result = Vec::with_capacity(carrier_wave_cycle.value());
                result.resize(carrier_wave_cycle.value(), 0.0);
                result
            },
        }
    }

    pub fn process_padded(&mut self, s: SamplesMut) {
        let samples = s.0;
        let len = self.buffer.len();
        let mut buffer = &mut self.buffer;
        let samples_length = samples.len();
        buffer[len / 2..len]
            .iter_mut()
            .zip(samples.iter())
            .for_each(|(b, s)| *b = *s);
        let mut max = buffer
            .iter()
            .map(|v| v.clone())
            .enumerate()
            .max_by(|lhs, rhs| lhs.1.partial_cmp(&rhs.1).unwrap())
            .unwrap();

        let start_samples_idx = len - len / 2;
        let mut buffer_rolling_idx = 0;
        for samples_idx in start_samples_idx..samples_length + start_samples_idx {
            buffer[buffer_rolling_idx] = if samples_idx < samples_length {
                samples[samples_idx]
            } else {
                0.0
            };
            if max.0 == buffer_rolling_idx {
                max = buffer
                    .iter()
                    .map(|v| v.clone())
                    .enumerate()
                    .max_by(|lhs, rhs| lhs.1.partial_cmp(&rhs.1).unwrap())
                    .unwrap();
            } else if buffer[buffer_rolling_idx] > max.1 {
                max = (buffer_rolling_idx, samples[samples_idx]);
            }
            samples[samples_idx - start_samples_idx] = max.1;
            buffer_rolling_idx += 1;
            if buffer_rolling_idx >= len {
                buffer_rolling_idx = 0;
            }
        }

        buffer.fill(0.0);
    }

    pub fn process(&mut self, s: SamplesMut) {
        let samples = s.0;
        let len = self.buffer.len();
        let mut buffer = &mut self.buffer;
        let samples_length = samples.len();
        buffer
            .iter_mut()
            .zip(samples.iter())
            .for_each(|(b, s)| *b = *s);
        let mut max = (0, 0.0f32);

        let start_samples_idx = len / 2;
        let mut buffer_rolling_idx = 0;
        for samples_idx in len..samples_length {
            buffer[buffer_rolling_idx] = samples[samples_idx];
            if max.0 == buffer_rolling_idx {
                max = buffer
                    .iter()
                    .map(|v| v.clone())
                    .enumerate()
                    .max_by(|lhs, rhs| lhs.1.partial_cmp(&rhs.1).unwrap())
                    .unwrap();
            } else if buffer[buffer_rolling_idx] > max.1 {
                max = (buffer_rolling_idx, samples[samples_idx]);
            }
            samples[samples_idx - start_samples_idx] = max.1;
            buffer_rolling_idx += 1;
            if buffer_rolling_idx >= len {
                buffer_rolling_idx = 0;
            }
        }

        buffer.fill(0.0);
    }
}

struct TransitionSearch {
    transition_offset: usize,
    signal_strength: f32,
}

impl TransitionSearch {
    pub fn full_search(
        s: Samples,
        transition_width: SampleCount,
        baud_width: SampleCount,
        min_snr: f32,
        transition: Transition,
    ) -> Self {
        let samples = s.0;
        todo!()
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_envelope_calculation_sawtooth() {
        let mut calc = EnvelopeCalculation::new(SampleCount::new(4));
        let mut buffer = [0.0f32, 1., 0., -1., 0., 1., 0., -1., 0.];
        calc.process_padded(SamplesMut(&mut buffer));
        assert_eq!(buffer[0], 1.0);
        assert_eq!(buffer[1], 1.0);
        assert_eq!(buffer[2], 1.0);
        assert_eq!(buffer[3], 1.0);
        assert_eq!(buffer[4], 1.0);
        assert_eq!(buffer[5], 1.0);
        assert_eq!(buffer[6], 1.0);
        assert_eq!(buffer[7], 0.0);
        assert_eq!(buffer[8], 0.0);
    }

    #[test]
    fn test_envelope_calculation_falling_ramp() {
        let mut calc = EnvelopeCalculation::new(SampleCount::new(4));
        let mut buffer = [1.0f32, 1., 1., 1., 0.5, 0., 0., 0., 0.];
        calc.process(SamplesMut(&mut buffer));
        assert_eq!(buffer[0], 1.0);
        assert_eq!(buffer[1], 1.0);
        assert_eq!(buffer[2], 1.0);
        assert_eq!(buffer[3], 1.0);
        assert_eq!(buffer[4], 1.0);
        assert_eq!(buffer[5], 0.5);
        assert_eq!(buffer[6], 0.0);
        assert_eq!(buffer[7], 0.0);
        assert_eq!(buffer[8], 0.0);
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
