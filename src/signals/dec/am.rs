use std::collections::VecDeque;

use crate::{
    sampling::{SampleCount, Samples, SamplingRate},
    signals::proc::{dft, FFT},
    units::{Amplitude, Frequency, Proportion, RationalFraction, Time},
    utils,
};

pub enum TransitionState {
    Hold(usize),
    Risng,
    Falling,
}

enum StateMachine {
    Searching,
    Synchronized(usize),
}

pub struct Parameters {
    carrier_frequency: Frequency,
    carrier_bandwidth: Frequency,
    sampling_rate: SamplingRate,
    fft_window_sc: SampleCount,
    max_trainsition_distance: usize,
    transition_convolution_kernels: (Box<[Amplitude]>, Box<[Amplitude]>, usize),
}

impl Parameters {
    pub fn new(
        carrier_frequency: Frequency,
        baudrate: Frequency,
        transition_width_proportion: Proportion,
        max_trainsition_distance: usize,
        sampling_rate: SamplingRate,
        transition_window_movement_divisor: usize,
    ) -> Self {
        let baud_length = baudrate.cycle_time();
        let transition_window_sample_count = sampling_rate * baud_length;
        Self {
            carrier_frequency: carrier_frequency,
            carrier_bandwidth: dft::step(
                transition_window_sample_count,
                sampling_rate.max_frequency(),
            ),
            sampling_rate: sampling_rate,
            fft_window_sc: transition_window_sample_count / transition_window_movement_divisor,
            max_trainsition_distance: max_trainsition_distance,
            transition_convolution_kernels: Self::transition_convolution_kernel(
                sampling_rate * baud_length,
                transition_width_proportion,
            ),
        }
    }

    pub fn transition_convolution_kernel(
        transition_window_sample_count: SampleCount,
        transition_proportion: Proportion,
    ) -> (Box<[Amplitude]>, Box<[Amplitude]>, usize) {
        let transition_length = std::cmp::max(
            (transition_window_sample_count * transition_proportion).value(),
            1usize,
        );
        let plateau_length = std::cmp::max(
            RationalFraction::new(1usize, 2usize) * transition_length,
            1usize,
        );

        let mut result = Vec::with_capacity(transition_window_sample_count.value());
        result.resize(transition_window_sample_count.value(), Amplitude::zero());
        result[0..plateau_length]
            .iter_mut()
            .for_each(|value| *value = Amplitude::new(1.0));
        result[transition_length - plateau_length..transition_length]
            .iter_mut()
            .for_each(|value| *value = Amplitude::new(-1.0));
        result[transition_length..transition_window_sample_count.value()]
            .iter_mut()
            .for_each(|value| {
                *value = Amplitude::new(
                    -1.0 / (transition_window_sample_count.value() - plateau_length) as f32,
                )
            });

        let rising_edge = result.clone().into_boxed_slice();
        result
            .iter_mut()
            .for_each(|value| *value = Amplitude::new(-value.value()));

        (rising_edge, result.into_boxed_slice(), plateau_length)
    }
}

struct State {
    realtime_backlog: std::sync::Mutex<VecDeque<f32>>,
    backlog: std::sync::Mutex<VecDeque<f32>>,
    monitor_windows: VecDeque<Amplitude>,
    signal_to_noise_ratio: Proportion,
    transitions: VecDeque<TransitionState>,
    sm: StateMachine,
    fft: FFT,
}

impl State {
    pub fn new() -> Self {
        Self {
            realtime_backlog: std::sync::Mutex::new(VecDeque::new()),
            backlog: std::sync::Mutex::new(VecDeque::new()),
            monitor_windows: VecDeque::new(),
            signal_to_noise_ratio: Proportion::zero(),
            transitions: VecDeque::new(),
            sm: StateMachine::Searching,
            fft: FFT::new(),
        }
    }
}

struct TransitionSearch {
    convolved: Box<[Amplitude]>,
    median: Amplitude,
    max: Amplitude,
}

impl TransitionSearch {
    pub fn process(signals: &[Amplitude], kernel: &[Amplitude]) -> Self {
        let res_len = utils::conv1d::valid_result_length(signals.len(), kernel.len());
        let mut res = Vec::with_capacity(res_len);
        res.resize(res_len, Amplitude::zero());
        let mut convolved = res.into_boxed_slice();
        utils::conv1d::valid(signals, kernel, &mut convolved).unwrap();

        let median = utils::median_non_averaged(&convolved).unwrap_or(Amplitude::zero());
        let max = *convolved
            .iter()
            .max_by(|lhs, rhs| lhs.partial_cmp(rhs).unwrap())
            .unwrap_or(&Amplitude::zero());

        Self {
            convolved: convolved,
            median: median,
            max: max,
        }
    }

    pub fn snr(&self) -> Proportion {
        let median = if self.median > Amplitude::zero() {
            self.median
        } else {
            Amplitude::new(f32::EPSILON)
        };
        self.max.relative_to(median)
    }

    pub fn max_index(&self) -> usize {
        self.convolved
            .iter()
            .enumerate()
            .find(|(idx, item)| item >= &&self.max)
            .unwrap()
            .0
            .clone()
    }
}

pub struct TransitionDecoder {
    c: Parameters,
    m: State,
}

impl TransitionDecoder {
    pub fn new(c: Parameters) -> Self {
        Self {
            c: c,
            m: State::new(),
        }
    }

    pub fn append_samples(&self, samples: Samples) {
        self.m
            .realtime_backlog
            .lock()
            .unwrap()
            .extend(samples.0.iter())
    }

    pub fn process(&mut self) {
        self.dequeue_realtime_samples();
        self.sample_backlog_to_windows();
    }

    pub fn parse(&mut self) {
        match self.m.sm {
            StateMachine::Searching => self.search(),
            _ => todo!(),
        }
    }

    fn search(&mut self) {
        self.m.monitor_windows.make_contiguous();

        let rising = TransitionSearch::process(
            self.m.monitor_windows.as_slices().0,
            &self.c.transition_convolution_kernels.0,
        );

        let falling = TransitionSearch::process(
            self.m.monitor_windows.as_slices().0,
            &self.c.transition_convolution_kernels.1,
        );
    }

    pub fn sample_backlog_to_windows(&mut self) {
        let samples_needed = self.c.fft_window_sc.value();
        let mut samples = self.m.backlog.lock().unwrap();
        let mut buffer = Vec::with_capacity(samples_needed);

        while samples.len() > samples_needed {
            buffer.clear();
            buffer.extend(samples.drain(0..samples_needed));
            let dft = self.m.fft.fft(Samples(&buffer), self.c.sampling_rate);
            self.m.monitor_windows.push_back(
                dft.absolute_amplitude_average_at(
                    self.c.carrier_frequency,
                    self.c.carrier_bandwidth,
                )
                .unwrap(),
            )
        }
    }

    pub fn dequeue_realtime_samples(&self) {
        let samples_needed = self.c.fft_window_sc.value();
        let mut source = self.m.realtime_backlog.lock().unwrap();
        let mut target = self.m.backlog.lock().unwrap();

        let samples_to_take = (source.len() / samples_needed) * samples_needed;
        target.extend(source.drain(0..samples_to_take));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn as_amplitudes(input: &[f32]) -> Box<[Amplitude]> {
        input
            .iter()
            .map(|val| Amplitude::new(*val))
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }

    #[test]
    pub fn edge_conv_kernel_test_0() {
        let (rising_kernel, falling_kernel, plateau_length) =
            Parameters::transition_convolution_kernel(SampleCount::new(10), Proportion::new(0.5));

        assert_eq!(
            rising_kernel,
            as_amplitudes(&[
                1.0,
                1.0,
                0.0,
                -1.0,
                -1.0,
                -1.0 / 8.0,
                -1.0 / 8.0,
                -1.0 / 8.0,
                -1.0 / 8.0,
                -1.0 / 8.0
            ])
        );
        assert_eq!(
            falling_kernel,
            as_amplitudes(&[
                -1.0,
                -1.0,
                0.0,
                1.0,
                1.0,
                1.0 / 8.0,
                1.0 / 8.0,
                1.0 / 8.0,
                1.0 / 8.0,
                1.0 / 8.0
            ])
        );
        assert_eq!(plateau_length, 2);
    }

    #[test]
    pub fn edge_conv_kernel_test_1() {
        let (rising_kernel, falling_kernel, plateau_length) =
            Parameters::transition_convolution_kernel(SampleCount::new(12), Proportion::new(0.5));

        assert_eq!(
            rising_kernel,
            as_amplitudes(&[
                1.0,
                1.0,
                1.0,
                -1.0,
                -1.0,
                -1.0,
                -1.0 / 9.0,
                -1.0 / 9.0,
                -1.0 / 9.0,
                -1.0 / 9.0,
                -1.0 / 9.0,
                -1.0 / 9.0
            ])
        );
        assert_eq!(
            falling_kernel,
            as_amplitudes(&[
                -1.0,
                -1.0,
                -1.0,
                1.0,
                1.0,
                1.0,
                1.0 / 9.0,
                1.0 / 9.0,
                1.0 / 9.0,
                1.0 / 9.0,
                1.0 / 9.0,
                1.0 / 9.0
            ])
        );
        assert_eq!(plateau_length, 3);
    }

    #[test]
    pub fn parameters_test_0() {
        let parameters = Parameters::new(
            Frequency::new(20000.0),
            Frequency::new(100.0),
            Proportion::new(0.25),
            5,
            SamplingRate::new(44100),
            32,
        );

        assert_eq!(parameters.carrier_frequency, Frequency::new(20000.0));
        assert_eq!(parameters.carrier_bandwidth, Frequency::new(25.0));
        assert_eq!(parameters.sampling_rate, SamplingRate::new(44100));
        assert_eq!(parameters.fft_window_sc, SampleCount::new(13));
        assert_eq!(parameters.max_trainsition_distance, 5);
        assert_eq!(parameters.transition_convolution_kernels.0.len(), 441);
        assert_eq!(parameters.transition_convolution_kernels.1.len(), 441);
    }

    #[test]
    pub fn parameters_test_1() {
        let parameters = Parameters::new(
            Frequency::new(20000.0),
            Frequency::new(1000.0),
            Proportion::new(0.25),
            5,
            SamplingRate::new(44100),
            32,
        );

        assert_eq!(parameters.carrier_frequency, Frequency::new(20000.0));
        assert_eq!(parameters.carrier_bandwidth, Frequency::new(245.0));
        assert_eq!(parameters.sampling_rate, SamplingRate::new(44100));
        assert_eq!(parameters.fft_window_sc, SampleCount::new(1));
        assert_eq!(parameters.max_trainsition_distance, 5);
        assert_eq!(parameters.transition_convolution_kernels.0.len(), 45);
        assert_eq!(parameters.transition_convolution_kernels.1.len(), 45);
    }

    #[test]
    pub fn transition_search_0_high_snr() {
        let signal: [f32; 8] = [0.1, 0.1, 0.2, 0.7, 1.0, 1.0, 0.9, 0.9];
        let kernel: [f32; 3] = [-1.0, 0.0, 1.0];
        let search = TransitionSearch::process(&as_amplitudes(&signal), &as_amplitudes(&kernel));
        assert_eq!(search.median, Amplitude::new(0.3));
        assert_eq!(search.max, Amplitude::new(0.8));
        assert_eq!(search.snr(), Proportion::new(2.6666665));
        assert_eq!(search.max_index(), 2);
    }

    #[test]
    pub fn transition_search_1_low_snr() {
        let signal: [f32; 8] = [0.98, 0.98, 0.98, 0.99, 1.0, 1.0, 1.0, 1.0];
        let kernel: [f32; 3] = [-1.0, 0.0, 1.0];
        let search = TransitionSearch::process(&as_amplitudes(&signal), &as_amplitudes(&kernel));
        assert_eq!(search.median, Amplitude::new(0.00999999));
        assert_eq!(search.max, Amplitude::new(0.01999998));
        assert_eq!(search.snr(), Proportion::new(2.0));
        assert_eq!(search.max_index(), 2);
    }

    #[test]
    pub fn transition_search_2_no_signal() {
        let signal: [f32; 8] = [1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0];
        let kernel: [f32; 3] = [-0.5, 0.0, 1.0];
        let search = TransitionSearch::process(&as_amplitudes(&signal), &as_amplitudes(&kernel));
        assert_eq!(search.median, Amplitude::new(0.5));
        assert_eq!(search.max, Amplitude::new(0.5));
        assert_eq!(search.snr(), Proportion::new(1.0));
        assert_eq!(search.max_index(), 0);
    }
}
