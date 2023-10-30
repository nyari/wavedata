use std::collections::VecDeque;

use crate::{
    sampling::{SampleCount, Samples, SamplingRate},
    signals::proc::FFT,
    units::{Amplitude, Frequency, Proportion, RationalFraction, Time},
};

pub enum TransitionState {
    WaitTrainsition,
    Transition,
    Hold,
}

enum StateMachine {
    Searching,
}

pub struct Parameters {
    carrier_frequency: Frequency,
    carrier_bandwidth: Frequency,
    max_trainsition_distance: usize,
    sampling_rate: SamplingRate,
    transition_sr: SamplingRate,
    transition_window_sample_count: SampleCount,
    transition_convolution_kernels: (Box<[Amplitude]>, Box<[Amplitude]>),
}

impl Parameters {
    pub fn new(
        carrier_frequency: Frequency,
        baudrate: Frequency,
        transition_width_proportion: Proportion,
        max_trainsition_distance: usize,
        sampling_rate: SamplingRate,
        transition_sr: SamplingRate,
        transition_window_movement_divisor: usize,
    ) -> Self {
        let baud_length = baudrate.cycle_time();
        let transition_width = baud_length * transition_width_proportion.value();
        let transition_window_sample_count =
            sampling_rate * transition_width / transition_window_movement_divisor;
        Self {
            carrier_frequency: carrier_frequency,
            carrier_bandwidth: Self::caluclate_bandwidth(carrier_frequency, transition_width),
            max_trainsition_distance: max_trainsition_distance,
            sampling_rate: sampling_rate,
            transition_sr: transition_sr,
            transition_window_sample_count: transition_window_sample_count,
            transition_convolution_kernels: Self::transition_convolution_kernel(
                transition_window_sample_count,
            ),
        }
    }

    pub fn caluclate_bandwidth(carrier_frequency: Frequency, transition_width: Time) -> Frequency {
        let transition_frequency = transition_width.frequency();
        if carrier_frequency < transition_frequency {
            carrier_frequency
        } else {
            transition_frequency
        }
    }

    pub fn transition_convolution_kernel(
        transition_window_sample_count: SampleCount,
    ) -> (Box<[Amplitude]>, Box<[Amplitude]>) {
        let plateau_length = std::cmp::max(
            RationalFraction::new(2usize, 10usize) * transition_window_sample_count.value(),
            1usize,
        );

        let mut result = Vec::with_capacity(transition_window_sample_count.value());
        result.resize(transition_window_sample_count.value(), Amplitude::zero());
        result[0..plateau_length]
            .iter_mut()
            .for_each(|value| *value = Amplitude::new(-1.0));
        result[transition_window_sample_count.value() - plateau_length
            ..transition_window_sample_count.value()]
            .iter_mut()
            .for_each(|value| *value = Amplitude::new(1.0));

        let rising_edge = result.clone().into_boxed_slice();
        result
            .iter_mut()
            .for_each(|value| *value = Amplitude::new(-value.value()));

        (rising_edge, result.into_boxed_slice())
    }
}

struct State {
    realtime_backlog: std::sync::Mutex<VecDeque<f32>>,
    backlog: std::sync::Mutex<VecDeque<f32>>,
    monitor_windows: VecDeque<Amplitude>,
    noise_level: Amplitude,
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
            noise_level: Amplitude::zero(),
            transitions: VecDeque::new(),
            sm: StateMachine::Searching,
            fft: FFT::new(),
        }
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
        }
    }

    fn search(&mut self) {}

    pub fn sample_backlog_to_windows(&mut self) {
        let samples_needed = self.c.transition_window_sample_count.value();
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
        let samples_needed = self.c.transition_window_sample_count.value();
        let mut source = self.m.realtime_backlog.lock().unwrap();
        let mut target = self.m.backlog.lock().unwrap();

        let samples_to_take = (source.len() / samples_needed) * samples_needed;
        target.extend(source.drain(0..samples_to_take));
    }
}
