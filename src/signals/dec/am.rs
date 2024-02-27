//! # Decode amplitude modulated signals
//!
//! ## Signal description
//!
use std::{cell::RefCell, collections::VecDeque, ops::Div, path::Ancestors};

use num::{bigint::Sign, complex::ComplexFloat};

use crate::{
    sampling::{SampleCount, Samples, SamplesMut, SamplingRate},
    signals::{am::Transition, proc::FFT},
    units::{Amplitude, Frequency, Proportion},
    utils::{self, Interval, WindowedWeightedAverage},
};

#[derive(Debug)]
enum SWError {
    NotEnoughSamples,
}

struct SignalWindow<'a> {
    samples: Samples<'a>,
    window: SampleCount,
    offset: SampleCount,
}

impl<'a> SignalWindow<'a> {
    pub fn new(s: Samples<'a>, window: SampleCount) -> Result<Self, SWError> {
        if window.value() <= s.0.len() && !s.0.is_empty() {
            Ok(Self {
                samples: s,
                window,
                offset: SampleCount::new(0),
            })
        } else {
            Err(SWError::NotEnoughSamples)
        }
    }

    pub fn end(&self) -> SampleCount {
        self.offset + self.window
    }

    pub fn begin(&self) -> SampleCount {
        self.offset
    }

    pub fn interval(&self) -> Interval<usize> {
        Interval::new(self.begin().value(), self.end().value())
    }

    pub fn delta(&self) -> f32 {
        let slice = self.slice();
        slice.0.last().unwrap() - slice.0.first().unwrap()
    }

    pub fn slice(&self) -> Samples<'a> {
        Samples(&self.samples.0[self.offset.value()..self.offset.value() + self.window.value()])
    }

    pub fn middle_index(&self) -> usize {
        self.offset.value() + self.window.value() / 2
    }

    pub fn middle_window(&self, samples: SampleCount) -> Result<Self, SWError> {
        let half = samples.value() / 2;
        let middle = self.middle_index();
        if middle >= half {
            let beg = middle - half;
            Ok(Self {
                samples: Samples(&self.samples.0[beg..beg + samples.value()]),
                window: samples,
                offset: SampleCount::new(0),
            })
        } else {
            Err(SWError::NotEnoughSamples)
        }
    }

    pub fn offset(self, offset: isize) -> Result<Self, SWError> {
        let old_offset = isize::try_from(self.offset.value()).unwrap();
        let new_offset = old_offset + offset;

        let test_offset_interval = Interval::new(
            0_isize,
            (self.samples.0.len() - self.window.value())
                .try_into()
                .unwrap(),
        );

        if test_offset_interval.in_co(&new_offset) {
            Ok(Self {
                offset: SampleCount::new(new_offset.try_into().unwrap()),
                ..self
            })
        } else {
            Err(SWError::NotEnoughSamples)
        }
    }

    pub fn next(self) -> Result<Self, SWError> {
        let offset = self.window.value().try_into().unwrap();
        self.offset(offset)
    }

    pub fn iter(&'a self) -> SignalWindows<'a> {
        SignalWindows { w: self.clone() }
    }
}

impl<'a> Clone for SignalWindow<'a> {
    fn clone(&self) -> Self {
        Self {
            samples: Samples(&self.samples.0),
            window: self.window,
            offset: self.offset,
        }
    }
}

struct SignalWindows<'a> {
    w: SignalWindow<'a>,
}

impl<'a> Iterator for SignalWindows<'a> {
    type Item = SignalWindow<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        match self.w.clone().next() {
            Ok(w) => {
                let result = self.w.clone();
                self.w = w;
                Some(result)
            },
            Err(SWError::NotEnoughSamples) => None,
            _ => panic!("Impossible case"),
        }
    }
}

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
    buffer_size: usize,
}

impl EnvelopeCalculation {
    pub fn new(carrier_wave_cycle: SampleCount) -> Self {
        Self {
            buffer_size: carrier_wave_cycle.value(),
        }
    }

    pub fn tail_lengths(&self) -> (usize, usize) {
        let remainder = self.buffer_size % 2;
        let half = self.buffer_size / 2;

        (half, half + remainder)
    }

    pub fn process_padded(&mut self, s: SamplesMut) {
        let samples = s.0;
        let len = self.buffer_size;
        let mut buffer = vec![0.0; len];
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
    }

    pub fn process(&mut self, s: SamplesMut) {
        let samples = s.0;
        let len = self.buffer_size;
        let mut buffer = vec![0.0; len];
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
    }
}

struct StartOfFrameSearch {
    transition_offset: usize,
    signal_level: f32,
    noise_level: f32,
}

impl StartOfFrameSearch {
    pub fn search_rising(
        s: Samples,
        transition_width: SampleCount,
        min_signal_level: f32,
    ) -> Option<Self> {
        let samples = s.0;

        let tw = transition_width.value() + 1;

        let mid_transition = samples
            .windows(tw)
            .map(|window| window.last().unwrap() - window.first().unwrap())
            .enumerate()
            .fold(None, |acc, (idx, signal)| {
                if let Some((first_idx, max_idx, old_signal)) = acc {
                    if idx - first_idx < tw && signal > old_signal {
                        Some((first_idx, idx, signal))
                    } else {
                        Some((first_idx, max_idx, old_signal))
                    }
                } else {
                    if signal > min_signal_level {
                        Some((idx, idx, signal))
                    } else {
                        None
                    }
                }
            });

        match mid_transition {
            Some((_, idx, signal)) => {
                let sum: f32 = samples[..idx].iter().sum();

                Some(Self {
                    transition_offset: idx,
                    signal_level: signal,
                    noise_level: sum / (idx as f32),
                })
            },
            None => None,
        }
    }
}

struct NextTransitionSearch {
    hold_length: usize,
    signal_level: Amplitude,
}

impl NextTransitionSearch {
    pub fn search(
        s: Samples,
        window_width: SampleCount,
        transition_width: SampleCount,
        transition_type: Transition,
        max_hold_length: usize,
        min_signal_level: Amplitude,
    ) -> Option<Self> {
        let mtp: f32 = match transition_type {
            Transition::Rising => 1.0,
            Transition::Falling => -1.0,
            _ => panic!("This is an incorrect case"),
        };

        SignalWindow::new(s, window_width)
            .unwrap()
            .iter()
            .enumerate()
            .take(max_hold_length + 1)
            .map(|(idx, win)| {
                (
                    idx,
                    win.middle_window(transition_width).unwrap().delta() * mtp,
                )
            })
            .find(|(_idx, signal_level)| signal_level > &min_signal_level.value())
            .map(|(hold_length, signal_level)| Self {
                hold_length,
                signal_level: Amplitude::new(signal_level),
            })
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
        assert_eq!(buffer, [1.0f32, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, 0.0]);
    }

    #[test]
    fn test_envelope_calculation_falling_ramp() {
        let mut calc = EnvelopeCalculation::new(SampleCount::new(4));
        let mut buffer = [1.0f32, 1., 1., 1., 0.5, 0., 0., 0., 0.];
        calc.process_padded(SamplesMut(&mut buffer));
        assert_eq!(buffer, [1.0f32, 1.0, 1.0, 1.0, 1.0, 0.5, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_envelope_calculation_sawtooth_no_padding() {
        let mut calc = EnvelopeCalculation::new(SampleCount::new(4));
        let mut buffer = [0.0f32, 1., 0., -1., 0., 1., 0., -1., 0.];
        calc.process(SamplesMut(&mut buffer));
        assert_eq!(buffer, [0.0f32, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, -1.0, 0.0]);
    }

    #[test]
    fn test_envelope_calculation_falling_ramp_no_padding() {
        let mut calc = EnvelopeCalculation::new(SampleCount::new(4));
        let mut buffer = [1.0f32, 1., 1., 1., 0.5, 0., 0., 0., 0.];
        calc.process(SamplesMut(&mut buffer));
        assert_eq!(buffer, [1.0f32, 1.0, 1.0, 1.0, 1.0, 0.5, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn start_of_frame_search_ramp_0_to_1_on_length_4() {
        let buffer = [0.0f32, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 1.0];
        let result =
            StartOfFrameSearch::search_rising(Samples(&buffer), SampleCount::new(4), 0.5).unwrap();

        assert_eq!(result.transition_offset, 2);
        assert_eq!(result.signal_level, 1.0);
        assert_eq!(result.noise_level, 0.0);
    }

    #[test]
    fn start_of_frame_search_ramp_0_to_1_on_length_4_under_signal_level() {
        let buffer = [0.0f32, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 1.0];
        let result = StartOfFrameSearch::search_rising(Samples(&buffer), SampleCount::new(4), 2.0);

        assert!(result.is_none());
    }

    #[test]
    fn start_of_frame_search_non_monotonous_ramp_0_to_1_on_length_6() {
        let buffer = [0.0f32, 0.0, 0.0, 0.25, 0.5, 0.25, 0.5, 0.75, 1.0, 1.0, 1.0];
        let result =
            StartOfFrameSearch::search_rising(Samples(&buffer), SampleCount::new(6), 0.5).unwrap();

        assert_eq!(result.transition_offset, 2);
        assert_eq!(result.signal_level, 1.0);
        assert_eq!(result.noise_level, 0.0);
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
        let mut result = vec![0.0f32; p.total_samples_count_estimate(message.len()).value()];

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
