use std::sync::{Arc, Mutex};

use rustfft::{num_complex::Complex, num_traits::Zero, Fft};

use crate::{
    sampling::{SampleCount, Samples, SamplingRate},
    units::{Amplitude, Frequency, Time},
};

pub enum Error {
    FrequencyOutOfBounds,
}

pub struct DFT {
    dft: Box<[Complex<f32>]>,
    length: Time,
}

impl DFT {
    pub fn new(dft: Box<[Complex<f32>]>, length: Time) -> Self {
        Self { dft: dft, length }
    }

    pub fn sample_count(&self) -> SampleCount {
        SampleCount::new(self.dft.len() / 2)
    }

    pub fn max_frequency(&self) -> Frequency {
        let sampling_rate = self.sample_count() / self.length;
        sampling_rate.max_frequency()
    }

    pub fn step(&self) -> Frequency {
        let sample_count = self.sample_count();
        let max = self.max_frequency();
        max / sample_count
    }

    pub fn band<'a>(
        &'a self,
        freq: Frequency,
        bandwidth: Frequency,
    ) -> Result<&'a [Complex<f32>], Error> {
        let step = self.step();
        let item = (freq / step) as usize;
        let radius = ((bandwidth / 2.0) / step) as usize;

        if SampleCount::new(item + radius) < self.sample_count() {
            Ok(&self.dft[item - radius..item + radius])
        } else {
            Err(Error::FrequencyOutOfBounds)
        }
    }

    pub fn absolute_amplitude_average_at(
        &self,
        freq: Frequency,
        bandwidth: Frequency,
    ) -> Result<Amplitude, Error> {
        let band = self.band(freq, bandwidth)?;
        let samples_count = band.len();
        Ok(band.iter().fold(Amplitude::new(0.0), |acc, elem| {
            acc + Amplitude::new(elem.re.abs())
        }) / (samples_count) as f32)
    }
}

pub struct FFT {
    ffts: Mutex<std::collections::HashMap<usize, Arc<dyn rustfft::Fft<f32>>>>,
}

impl FFT {
    pub fn new() -> Self {
        Self {
            ffts: Mutex::new(std::collections::HashMap::new()),
        }
    }

    pub fn fft(&self, s: Samples, rate: SamplingRate) -> DFT {
        let fft: Arc<dyn Fft<f32>> = self.getfft(s.0.len());
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
        DFT::new(output.into_boxed_slice(), s.count() / rate)
    }

    pub fn fft_inverse(&self, s: &[Complex<f32>]) -> Box<[Complex<f32>]> {
        todo!()
    }

    fn getfft(&self, len: usize) -> Arc<dyn rustfft::Fft<f32>> {
        let mut ffts = self.ffts.lock().unwrap();
        let result = ffts.get(&len);
        match result {
            Some(value) => value.clone(),
            None => {
                let mut planner = rustfft::FftPlanner::new();
                let instance = planner.plan_fft_forward(len);
                ffts.insert(len, instance.clone());
                instance
            },
        }
    }
}
