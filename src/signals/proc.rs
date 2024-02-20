use std::sync::{Arc, Mutex};

use num::complex::ComplexFloat;
use rustfft::{num_complex::Complex, num_traits::Zero, Fft};

use crate::{
    sampling::{Samples, SamplingRate},
    units::{Amplitude, Frequency},
};

#[derive(Debug)]
pub enum Error {
    FrequencyOutOfBounds,
}

pub mod dft {
    use super::Frequency;

    pub fn step(frequency_steps: usize, max_frequency: Frequency) -> Frequency {
        max_frequency.recip_scale(frequency_steps as f32)
    }
}

pub struct DFT {
    dft: Vec<Complex<f32>>,
    rate: SamplingRate,
}

impl DFT {
    pub fn new(dft: Vec<Complex<f32>>, rate: SamplingRate) -> Self {
        Self { dft, rate }
    }

    pub fn frequency_steps(&self) -> usize {
        self.dft.len() / 2
    }

    pub fn max_frequency(&self) -> Frequency {
        self.rate.max_frequency()
    }

    pub fn step(&self) -> Frequency {
        dft::step(self.frequency_steps(), self.max_frequency())
    }

    pub fn as_slice<'a>(&'a self) -> &'a [Complex<f32>] {
        self.dft.as_slice()
    }

    pub fn as_mut_slice<'a>(&'a mut self) -> &'a mut [Complex<f32>] {
        self.dft.as_mut_slice()
    }

    pub fn band<'a>(
        &'a self,
        freq: Frequency,
        bandwidth: Frequency,
    ) -> Result<&'a [Complex<f32>], Error> {
        let step = self.step();
        let steps = bandwidth.bandwidth_steps(step);
        self.band_steps(freq, steps)
    }

    pub fn band_step_bounds(&self, freq: Frequency, steps: usize) -> Result<(usize, usize), Error> {
        let step = self.step();
        let item = (freq / step).round() as usize;
        if item < self.frequency_steps() {
            let radius = steps;
            let lower_bound = if item >= radius { item - radius } else { 0 };
            let upper_bound = std::cmp::min(item + radius + 1, self.frequency_steps());

            Ok((lower_bound, upper_bound))
        } else {
            Err(Error::FrequencyOutOfBounds)
        }
    }

    pub fn band_steps<'a>(
        &'a self,
        freq: Frequency,
        steps: usize,
    ) -> Result<&'a [Complex<f32>], Error> {
        self.band_step_bounds(freq, steps)
            .map(|(lower, upper)| &self.dft[lower..upper])
    }

    pub fn filter_band(&mut self, freq: Frequency, bandwidth: Frequency) -> Result<(), Error> {
        let step = self.step();
        let steps = bandwidth.bandwidth_steps(step);
        let (lower, upper) = self.band_step_bounds(freq, steps)?;
        let len = self.dft.len();
        self.dft[0..lower].fill(Complex::zero());
        self.dft[upper..(len - upper)].fill(Complex::zero());
        self.dft[(len - lower)..len].fill(Complex::zero());
        Ok(())
    }

    pub fn band_average_amplitude<'a>(&'a self, band: &'a [Complex<f32>]) -> Amplitude {
        let samples_count = band.len();
        band.iter()
            .fold(Amplitude::new(0.0), |acc, elem| {
                acc + Amplitude::new(elem.abs())
            })
            .recip_scale((samples_count) as f32)
    }

    pub fn absolute_amplitude_average_at(
        &self,
        freq: Frequency,
        bandwidth: Frequency,
    ) -> Result<Amplitude, Error> {
        Ok(self.band_average_amplitude(self.band(freq, bandwidth)?))
    }

    pub fn absolute_amplitude_average_bwsteps_at(
        &self,
        freq: Frequency,
        steps: usize,
    ) -> Result<Amplitude, Error> {
        Ok(self.band_average_amplitude(self.band_steps(freq, steps)?))
    }
}

pub struct FFT {
    ffts: Mutex<std::collections::HashMap<usize, Arc<dyn rustfft::Fft<f32>>>>,
    iffts: Mutex<std::collections::HashMap<usize, Arc<dyn rustfft::Fft<f32>>>>,
}

impl FFT {
    pub fn new() -> Self {
        Self {
            ffts: Mutex::new(std::collections::HashMap::new()),
            iffts: Mutex::new(std::collections::HashMap::new()),
        }
    }

    pub fn fft(&self, s: Samples, rate: SamplingRate) -> DFT {
        let fft: Arc<dyn Fft<f32>> = self.getfft(s.0.len());
        let mut input: Vec<_> = s.0.iter().map(|x| Complex::new(x.clone(), 0.0)).collect();
        let (mut output, mut scratch) = {
            let mut buffer = Vec::new();
            buffer.resize(s.0.len(), Complex::zero());
            let output = buffer.clone();
            buffer.resize(fft.get_outofplace_scratch_len(), Complex::zero());
            (output, buffer)
        };
        fft.process_outofplace_with_scratch(
            input.as_mut_slice(),
            output.as_mut_slice(),
            scratch.as_mut_slice(),
        );
        DFT::new(output, rate)
    }

    pub fn fft_inverse(&self, input: &mut [Complex<f32>]) -> Vec<Complex<f32>> {
        let ifft: Arc<dyn Fft<f32>> = self.getifft(input.len());
        let mut output = vec![Complex::zero(); input.len()];
        let mut scratch = vec![Complex::zero(); ifft.get_outofplace_scratch_len()];
        ifft.process_outofplace_with_scratch(input, output.as_mut_slice(), scratch.as_mut_slice());
        output
    }

    fn getfft(&self, len: usize) -> Arc<dyn rustfft::Fft<f32>> {
        let mut ffts = self.ffts.lock().unwrap();
        let result = ffts.get(&len);
        match result {
            Some(value) => value.clone(),
            None => {
                let mut planner = rustfft::FftPlanner::new();
                let forward = planner.plan_fft_forward(len);
                ffts.insert(len, forward.clone());
                forward
            },
        }
    }

    fn getifft(&self, len: usize) -> Arc<dyn rustfft::Fft<f32>> {
        let mut iffts = self.iffts.lock().unwrap();
        let result = iffts.get(&len);
        match result {
            Some(value) => value.clone(),
            None => {
                let mut planner = rustfft::FftPlanner::new();
                let forward = planner.plan_fft_inverse(len);
                iffts.insert(len, forward.clone());
                forward
            },
        }
    }
}
