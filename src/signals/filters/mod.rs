use std::sync::Arc;

use rustfft::{num_complex::Complex, num_traits::Zero};

use crate::{
    sampling::{Samples, SamplingRate},
    units::Frequency,
};

pub struct FrequencyFilter {
    f: Frequency,
    bw: Frequency,
    ffts: std::collections::HashMap<usize, Arc<dyn rustfft::Fft<f32>>>,
}

impl FrequencyFilter {
    pub fn new(f: Frequency, bw: Frequency) -> Self {
        Self {
            f: f,
            bw: bw,
            ffts: std::collections::HashMap::new(),
        }
    }

    pub fn filter(&self, s: Samples, rate: SamplingRate) -> Box<Samples> {
        todo!()
    }

    pub fn fft(&mut self, s: Samples) -> Box<[Complex<f32>]> {
        let mut fft = self.getfft(s.0.len());
        let mut input: Vec<_> = s.0.iter().map(|x| Complex::new(x.clone(), 0.0)).collect();
        let (mut output, mut scratch) = {
            let mut buffer = Vec::new();
            buffer.resize(s.0.len(), Complex::zero());
            (buffer.clone(), buffer)
        };
        fft.process_outofplace_with_scratch(
            input.as_mut_slice(),
            output.as_mut_slice(),
            scratch.as_mut_slice(),
        );
        output.into_boxed_slice()
    }

    pub fn getfft(&mut self, len: usize) -> Arc<dyn rustfft::Fft<f32>> {
        let result = self.ffts.get(&len);
        match result {
            Some(value) => value.clone(),
            None => {
                let mut planner = rustfft::FftPlanner::new();
                let instance = planner.plan_fft_forward(len);
                self.ffts.insert(len, instance.clone());
                instance
            },
        }
    }
}
