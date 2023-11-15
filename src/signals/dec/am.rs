use std::collections::VecDeque;

use crate::{
    sampling::{SampleCount, Samples, SamplingRate},
    signals::proc::{dft, FFT},
    units::{Amplitude, Frequency, Proportion, RationalFraction},
    utils,
};

#[derive(Debug, Clone, Copy)]
enum Error {
    IncorrectTransition,
}

#[derive(Clone, Copy)]
pub enum TransitionState {
    Hold(usize),
    Rising,
    Falling,
    Noise(usize),
}

impl TransitionState {
    pub fn transition(&mut self, ts: Self) {
        *self = match (*self, ts) {
            (Self::Rising, Self::Rising) => Self::Noise(0),
            (Self::Falling, Self::Falling) => Self::Noise(0),
            (Self::Noise(pre), Self::Noise(add)) => Self::Noise(pre + add),
            (Self::Hold(pre), Self::Hold(add)) => Self::Hold(pre + add),
            _ => ts,
        }
    }
}

#[derive(Clone, Copy)]
enum StateMachine {
    Searching,
    Synchronized,
}

pub struct Parameters {
    carrier_frequency: Frequency,
    carrier_bandwidth: Frequency,
    sampling_rate: SamplingRate,
    fft_window_sc: SampleCount,
    max_trainsition_distance: usize,
    transition_convolution_kernels: (Box<[Amplitude]>, Box<[Amplitude]>, usize),
    min_snr: Proportion,
}

impl Parameters {
    pub fn new(
        carrier_frequency: Frequency,
        baudrate: Frequency,
        transition_width_proportion: Proportion,
        max_trainsition_distance: usize,
        sampling_rate: SamplingRate,
        transition_window_movement_divisor: usize,
        min_snr: Proportion,
    ) -> Self {
        let baud_length = baudrate.cycle_time();
        let transition_window_sample_count = sampling_rate * baud_length;
        let fft_window_sc = transition_window_sample_count / transition_window_movement_divisor;
        Self {
            carrier_frequency: carrier_frequency,
            carrier_bandwidth: dft::step(
                transition_window_sample_count,
                sampling_rate.max_frequency(),
            ),
            sampling_rate: sampling_rate,
            fft_window_sc: fft_window_sc,
            max_trainsition_distance: max_trainsition_distance,
            transition_convolution_kernels: Self::transition_convolution_kernel(
                SampleCount::new(transition_window_movement_divisor),
                transition_width_proportion,
            ),
            min_snr: min_snr,
        }
    }

    pub fn buad_length(&self) -> usize {
        self.transition_convolution_kernels.0.len()
    }

    pub fn transition_convolution_kernel(
        transition_window_fft_sample_count: SampleCount,
        transition_proportion: Proportion,
    ) -> (Box<[Amplitude]>, Box<[Amplitude]>, usize) {
        let transition_length = std::cmp::max(
            (transition_window_fft_sample_count * transition_proportion).value(),
            1usize,
        );
        let plateau_length = std::cmp::max(
            RationalFraction::new(1usize, 2usize) * transition_length,
            1usize,
        );

        let mut result = Vec::with_capacity(transition_window_fft_sample_count.value());
        result.resize(
            transition_window_fft_sample_count.value(),
            Amplitude::zero(),
        );
        result[0..plateau_length]
            .iter_mut()
            .for_each(|value| *value = Amplitude::new(1.0));
        result[transition_length - plateau_length..transition_length]
            .iter_mut()
            .for_each(|value| *value = Amplitude::new(-1.0));
        result[transition_length..transition_window_fft_sample_count.value()]
            .iter_mut()
            .for_each(|value| {
                *value = Amplitude::new(
                    -1.0 / (transition_window_fft_sample_count.value() - plateau_length) as f32,
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
    carrier_amplitudes: VecDeque<Amplitude>,
    transitions: VecDeque<TransitionState>,
    sm: StateMachine,
    fft: FFT,
}

impl State {
    fn new() -> Self {
        Self {
            realtime_backlog: std::sync::Mutex::new(VecDeque::new()),
            backlog: std::sync::Mutex::new(VecDeque::new()),
            carrier_amplitudes: VecDeque::new(),
            transitions: VecDeque::new(),
            sm: StateMachine::Searching,
            fft: FFT::new(),
        }
    }

    fn push_transition(&mut self, ts: TransitionState) -> TransitionState {
        match self.transitions.back_mut() {
            Some(last) => last.transition(ts),
            None => self.transitions.push_back(ts),
        }
        *self.transitions.back().unwrap()
    }

    fn parse_traisition(&mut self, ts: TransitionState) {
        self.sm = match (self.sm, ts) {
            (StateMachine::Searching, TransitionState::Rising) => {
                self.push_transition(TransitionState::Rising);
                StateMachine::Synchronized
            },
            (StateMachine::Searching, TransitionState::Noise(_)) => StateMachine::Searching,
            (StateMachine::Searching, _) => {
                panic!("Incorrect internal state... Searching only accepts rising transition")
            },
            (StateMachine::Synchronized, change) => match self.push_transition(change) {
                TransitionState::Noise(_) => StateMachine::Searching,
                _ => StateMachine::Synchronized,
            },
        }
    }

    fn last_transition(&self) -> TransitionState {
        match self.transitions.back() {
            Some(ts) => *ts,
            _ => TransitionState::Noise(0),
        }
    }

    fn drain_carrier_amplitudes(&mut self, amount: usize) {
        self.carrier_amplitudes.drain(..amount).for_each(|_| {});
    }
}

struct TransitionSearch {
    convolved: Box<[Amplitude]>,
    median: Amplitude,
    max: Amplitude,
    signal_beg_offset: usize,
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
            signal_beg_offset: signals.len() - res_len,
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

    pub fn signal_start_idx(&self) -> usize {
        self.signal_beg_offset + self.max_index()
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
        self.sample_backlog_to_carrier_amplitudes();
    }

    pub fn parse(&mut self) {
        match self.m.sm {
            StateMachine::Searching => self.search(),
            StateMachine::Synchronized => self.next_baud(),
        }
    }

    fn search_transition(
        &self,
        rising: TransitionSearch,
        falling: TransitionSearch,
    ) -> (TransitionState, usize) {
        let transition = if rising.snr() > self.c.min_snr {
            (TransitionState::Rising, Some(rising))
        } else if falling.snr() > self.c.min_snr {
            (TransitionState::Falling, Some(falling))
        } else {
            (TransitionState::Hold(1), None)
        };

        match transition {
            (ts, Some(edge)) => (ts, edge.signal_start_idx()),
            (ts, None) => (ts, 0),
        }
    }

    fn next_baud(&mut self) {
        self.m.carrier_amplitudes.make_contiguous();

        let rising = TransitionSearch::process(
            &self.m.carrier_amplitudes.as_slices().0[..self.c.buad_length()],
            &self.c.transition_convolution_kernels.0,
        );

        let falling = TransitionSearch::process(
            &self.m.carrier_amplitudes.as_slices().0[..self.c.buad_length()],
            &self.c.transition_convolution_kernels.1,
        );

        let (ts, sync_offset) = self.search_transition(rising, falling);

        self.m
            .drain_carrier_amplitudes(self.c.buad_length() + sync_offset - 1);

        self.m.parse_traisition(ts);

        self.handle_synchronization();
    }

    fn handle_synchronization(&mut self) {
        match self.m.last_transition() {
            TransitionState::Hold(value) if value >= self.c.max_trainsition_distance => {
                self.m.parse_traisition(TransitionState::Noise(1));
            },
            _ => (),
        }
    }

    fn search(&mut self) {
        self.m.carrier_amplitudes.make_contiguous();

        let rising = TransitionSearch::process(
            self.m.carrier_amplitudes.as_slices().0,
            &self.c.transition_convolution_kernels.0,
        );

        if rising.snr() > self.c.min_snr {
            self.m.drain_carrier_amplitudes(rising.signal_start_idx());
            self.m.parse_traisition(TransitionState::Rising);
        }
    }

    pub fn sample_backlog_to_carrier_amplitudes(&mut self) {
        let samples_needed = self.c.fft_window_sc.value();
        let mut samples = self.m.backlog.lock().unwrap();
        let mut buffer = Vec::with_capacity(samples_needed);

        while samples.len() > samples_needed {
            buffer.clear();
            buffer.extend(samples.drain(0..samples_needed));
            let dft = self.m.fft.fft(Samples(&buffer), self.c.sampling_rate);
            self.m.carrier_amplitudes.push_back(
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
            Proportion::new(5.0),
        );

        assert_eq!(parameters.carrier_frequency, Frequency::new(20000.0));
        assert_eq!(parameters.carrier_bandwidth, Frequency::new(25.0));
        assert_eq!(parameters.sampling_rate, SamplingRate::new(44100));
        assert_eq!(parameters.fft_window_sc, SampleCount::new(13));
        assert_eq!(parameters.max_trainsition_distance, 5);
        assert_eq!(parameters.transition_convolution_kernels.0.len(), 32);
        assert_eq!(parameters.transition_convolution_kernels.1.len(), 32);
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
            Proportion::new(5.0),
        );

        assert_eq!(parameters.carrier_frequency, Frequency::new(20000.0));
        assert_eq!(parameters.carrier_bandwidth, Frequency::new(245.0));
        assert_eq!(parameters.sampling_rate, SamplingRate::new(44100));
        assert_eq!(parameters.fft_window_sc, SampleCount::new(1));
        assert_eq!(parameters.max_trainsition_distance, 5);
        assert_eq!(parameters.transition_convolution_kernels.0.len(), 32);
        assert_eq!(parameters.transition_convolution_kernels.1.len(), 32);
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

#[cfg(test)]
mod integration_test {
    use crate::{
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
        fn total_samples_count(&self, message_len: usize) -> SampleCount {
            self.sampling_rate * (self.lead_in + self.lead_out)
                + (self.sampling_rate * (self.baudrate.cycle_time() * 2.0)) * message_len
        }

        fn lead_in_sample_count(&self) -> SampleCount {
            self.sampling_rate * self.lead_in
        }

        fn create_parameters(&self) -> Parameters {
            Parameters::new(
                self.carrier_frequency,
                self.baudrate,
                self.transition_width,
                self.stuff_bit as usize,
                self.sampling_rate,
                8,
                Proportion::new(10.0),
            )
        }
    }

    fn create_signal_with_message(message: &str, p: &Params) -> Vec<f32> {
        let mut result = Vec::with_capacity(p.total_samples_count(message.len()).value());
        result.resize(p.total_samples_count(message.len()).value(), 0.0);

        let carrier_signal = crate::sampling::WaveSampler::new(crate::waves::Sine::new(
            p.carrier_frequency,
            Time::zero(),
            p.carrier_amplitude,
        ));
        let data_signal = crate::sampling::SignalSampler::new(crate::signals::enc::am::NRZ::new(
            crate::signals::enc::am::NRZConsts::new(p.baudrate, p.transition_width, p.high_low),
            crate::encodings::enc::nrz::Parameters::new(
                message.as_bytes().iter().map(|x| x.clone()).collect(),
                p.stuff_bit,
            ),
        ));
        let mut composite_sampler =
            crate::sampling::CompositeSampler::new(carrier_signal, data_signal, |input, output| {
                *output = input.0 * input.1;
            });

        composite_sampler.sample_into_f32(
            SamplesMut(&mut result[p.lead_in_sample_count().value()..]),
            p.sampling_rate,
        );

        result
    }

    #[test]
    fn integration_test_1() {
        let p = Params {
            lead_in: Time::new(1.0),
            lead_out: Time::new(1.0),
            carrier_frequency: Frequency::new(20000.0),
            sampling_rate: SamplingRate::new(44100),
            carrier_amplitude: Amplitude::new(1.0),
            baudrate: Frequency::new(100.0),
            transition_width: Proportion::new(0.25),
            high_low: (Amplitude::new(1.0), Amplitude::new(0.0)),
            stuff_bit: 4,
        };

        let input = create_signal_with_message("Hello world", &p);
        let mut decoder = TransitionDecoder::new(p.create_parameters());

        decoder.append_samples(Samples(input.as_slice()));
        decoder.process();
        decoder.parse();

        assert_ne!(decoder.m.transitions.len(), 0);
    }
}
