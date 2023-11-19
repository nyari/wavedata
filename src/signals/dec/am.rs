use std::collections::VecDeque;

use crate::{
    sampling::{SampleCount, Samples, SamplingRate},
    signals::proc::FFT,
    units::{Amplitude, Frequency, Proportion},
    utils,
};

#[derive(Debug, Clone, Copy)]
enum Error {
    IncorrectTransition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionState {
    Hold(usize),
    Rising,
    Falling,
    Noise(usize),
}

#[derive(Clone, Copy)]
enum StateMachine {
    Searching,
    Synchronized,
}

struct TransitionSearchParams {
    transition_width: usize,
    half_window_width: usize,
    window_width: usize,
    monitor_width: usize,
    kernel: Box<[Amplitude]>,
    min_snr: Proportion,
}

impl TransitionSearchParams {
    pub fn create(transition_width: usize, window_width: usize, min_snr: Proportion) -> Self {
        let mut kernel = Vec::with_capacity(transition_width);
        kernel.resize(transition_width, Amplitude::zero());
        kernel[0] = Amplitude::new(-1.0);
        kernel[transition_width - 1] = Amplitude::new(1.0);
        Self {
            transition_width,
            half_window_width: window_width / 2,
            window_width,
            monitor_width: window_width * 2,
            kernel: kernel.into_boxed_slice(),
            min_snr,
        }
    }
}

struct TransitionSearch {
    snr: Proportion,
    ts: TransitionState,
    sig_begin_offset: usize,
    mid_transition_window_offset: usize,
    transitionless_windows: usize,
}

impl TransitionSearch {
    pub fn search(p: &TransitionSearchParams, signals: &[Amplitude]) -> Option<Self> {
        let conv_res_length = utils::conv1d::valid_result_length(signals.len(), p.kernel.len());
        let conv = {
            let mut res = Vec::new();
            res.resize(conv_res_length, Amplitude::zero());
            utils::conv1d::valid(signals, &p.kernel, &mut res).unwrap();
            res
        };
        let abs_conv: Vec<_> = conv.iter().map(|i| i.abs()).collect();

        let median = utils::median_non_averaged(&abs_conv).unwrap().abs();
        let max = abs_conv
            .iter()
            .max_by(|lhs, rhs| lhs.abs().partial_cmp(&rhs.abs()).unwrap())
            .unwrap();

        let snr = max.relative_to(median);

        if snr >= p.min_snr {
            let (idx, _) = abs_conv
                .windows(3)
                .enumerate()
                .find(|(_, win)| win[1].abs().relative_to(median) > p.min_snr && utils::nms(win))
                .unwrap();

            let ts = if conv[idx + 1]
                .partial_cmp(&Amplitude::zero())
                .unwrap()
                .is_gt()
            {
                TransitionState::Rising
            } else {
                TransitionState::Falling
            };

            let sig_begin_offset = idx + 1;

            Some(Self {
                snr,
                ts,
                sig_begin_offset,
                mid_transition_window_offset: sig_begin_offset + p.half_window_width,
                transitionless_windows: sig_begin_offset / p.window_width,
            })
        } else {
            None
        }
    }
}

pub struct Parameters {
    carrier_frequency: Frequency,
    sampling_rate: SamplingRate,
    fft_window_sc: SampleCount,
    max_transitionless_windows: usize,
    transiton_searc_params: TransitionSearchParams,
}

impl Parameters {
    pub fn new(
        carrier_frequency: Frequency,
        baudrate: Frequency,
        transition_width_proportion: Proportion,
        max_transitionless_windows: usize,
        sampling_rate: SamplingRate,
        transition_window_movement_divisor: usize,
        min_snr: Proportion,
    ) -> Self {
        let baud_length = baudrate.cycle_time();
        let transition_window_sample_count = sampling_rate * baud_length;
        let fft_window_sc = transition_window_sample_count / transition_window_movement_divisor;
        Self {
            carrier_frequency: carrier_frequency,
            sampling_rate: sampling_rate,
            fft_window_sc: fft_window_sc,
            max_transitionless_windows: max_transitionless_windows,
            transiton_searc_params: TransitionSearchParams::create(
                transition_width_proportion.scale_usize(transition_window_movement_divisor),
                transition_window_movement_divisor,
                min_snr,
            ),
        }
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

enum PushOp {
    Push(TransitionState),
    Mutate(TransitionState),
    Skip,
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
        let decision = match (self.transitions.back(), ts) {
            (_, TransitionState::Noise(0)) => PushOp::Skip,
            (_, TransitionState::Hold(0)) => PushOp::Skip,
            (None, ts) => PushOp::Push(ts),
            (Some(TransitionState::Noise(pre)), TransitionState::Noise(post)) => {
                PushOp::Mutate(TransitionState::Noise(pre + post))
            },
            (Some(TransitionState::Hold(pre)), TransitionState::Hold(post)) => {
                PushOp::Mutate(TransitionState::Hold(pre + post))
            },
            (Some(a), b) if *a == b => PushOp::Push(TransitionState::Noise(1)),
            _ => PushOp::Push(ts),
        };

        match decision {
            PushOp::Push(ts) => self.transitions.push_back(ts),
            PushOp::Mutate(ts) => *self.transitions.back_mut().unwrap() = ts,
            PushOp::Skip => (),
        };

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
        while self.m.carrier_amplitudes.len() > self.c.transiton_searc_params.window_width {
            match self.m.sm {
                StateMachine::Searching => self.search(),
                StateMachine::Synchronized => self.next_baud(),
            }
        }
    }

    fn next_baud(&mut self) {
        self.m.carrier_amplitudes.make_contiguous();
        let hold_window_size =
            self.c.transiton_searc_params.window_width * (self.c.max_transitionless_windows + 1);

        let hold_window = utils::begin_upper_limit_slice(
            self.m.carrier_amplitudes.as_slices().0,
            hold_window_size,
        );

        match TransitionSearch::search(&self.c.transiton_searc_params, hold_window) {
            Some(ts) => {
                self.m
                    .parse_traisition(TransitionState::Hold(ts.transitionless_windows));
                self.m.parse_traisition(ts.ts);
                self.m
                    .drain_carrier_amplitudes(ts.mid_transition_window_offset);
            },
            None => {
                if hold_window.len() >= hold_window_size {
                    self.m.parse_traisition(TransitionState::Noise(1))
                }
            },
        }
    }

    fn search(&mut self) {
        self.m.carrier_amplitudes.make_contiguous();
        let ts = {
            let signals = self.m.carrier_amplitudes.as_slices().0;
            TransitionSearch::search(&self.c.transiton_searc_params, signals)
        };

        match ts {
            Some(res) => {
                self.m.parse_traisition(TransitionState::Rising);
                self.m
                    .drain_carrier_amplitudes(res.mid_transition_window_offset);
            },
            None => {
                self.m.drain_carrier_amplitudes(
                    self.m.carrier_amplitudes.len()
                        - self.c.transiton_searc_params.half_window_width,
                );
            },
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
                dft.absolute_amplitude_average_bwsteps_at(self.c.carrier_frequency, 0)
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
    pub fn parameters_test_0() {
        let parameters = Parameters::new(
            Frequency::new(20000.0),
            Frequency::new(100.0),
            Proportion::new(0.25),
            5,
            SamplingRate::new(44100),
            8,
            Proportion::new(5.0),
        );

        assert_eq!(parameters.carrier_frequency, Frequency::new(20000.0));
        assert_eq!(parameters.sampling_rate, SamplingRate::new(44100));
        assert_eq!(parameters.fft_window_sc, SampleCount::new(13));
        assert_eq!(parameters.max_transitionless_windows, 5);
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
        assert_eq!(parameters.sampling_rate, SamplingRate::new(44100));
        assert_eq!(parameters.fft_window_sc, SampleCount::new(1));
        assert_eq!(parameters.max_transitionless_windows, 5);
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
                + (self.sampling_rate * (self.baudrate.cycle_time().mul(2.0))) * message_len
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
                Proportion::new(5.0),
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
            lead_in: Time::new(0.5),
            lead_out: Time::new(0.5),
            carrier_frequency: Frequency::new(20000.0),
            sampling_rate: SamplingRate::new(44100),
            carrier_amplitude: Amplitude::new(1.0),
            baudrate: Frequency::new(100.0),
            transition_width: Proportion::new(0.5),
            high_low: (Amplitude::new(1.0), Amplitude::new(0.0)),
            stuff_bit: 4,
        };

        let input = create_signal_with_message("ABCD", &p);
        let mut decoder = TransitionDecoder::new(p.create_parameters());

        decoder.append_samples(Samples(input.as_slice()));
        decoder.process();
        decoder.parse();

        assert_ne!(decoder.m.transitions.len(), 0);
    }
}
