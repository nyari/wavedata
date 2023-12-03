pub mod conv1d {
    #[derive(Debug)]
    pub enum Error {
        IncorrectOutputSize,
        SignalShorterThanKernel,
    }

    pub fn same<T>(signal: &[T], kernel: &[T], result: &mut [T])
    where
        T: std::ops::Add<T, Output = T> + std::ops::Mul<T, Output = T> + num::traits::Zero + Clone,
    {
        if signal.len() != result.len() {
            panic!("Input signal and result output lenghts differ for convolution");
        }

        let half_kernel_len = (kernel.len() / 2) as isize;

        result
            .iter_mut()
            .enumerate()
            .for_each(|(idxusize, result_elem)| {
                let idx = idxusize as isize;

                let signal_start_idx = std::cmp::max(0, idx - half_kernel_len) as usize;
                let kernel_start_idx = std::cmp::max(half_kernel_len - idx, 0) as usize;
                let kernel_end_idx = std::cmp::min(
                    kernel.len(),
                    signal.len() + (half_kernel_len as usize) - idxusize,
                );

                for kernel_idx in kernel_start_idx..kernel_end_idx {
                    let signal_elem =
                        signal[signal_start_idx + (kernel_idx - kernel_start_idx)].clone();
                    let kernel_elem = kernel[kernel_idx].clone();
                    *result_elem = result_elem.clone() + (signal_elem * kernel_elem);
                }
            })
    }

    pub fn valid<T>(signal: &[T], kernel: &[T], result: &mut [T]) -> Result<(), Error>
    where
        T: std::ops::Add<T, Output = T> + std::ops::Mul<T, Output = T> + num::traits::Zero + Clone,
    {
        if signal.len() < kernel.len() {
            Err(Error::SignalShorterThanKernel)
        } else if result.len() != valid_result_length(signal.len(), kernel.len()) {
            Err(Error::IncorrectOutputSize)
        } else {
            signal
                .windows(kernel.len())
                .zip(result.iter_mut())
                .for_each(|(window, result)| {
                    *result = window
                        .iter()
                        .zip(kernel.iter())
                        .fold(T::zero(), |acc, (sig, ker)| acc + sig.clone() * ker.clone());
                });
            Ok(())
        }
    }

    pub fn valid_result_length(signal: usize, kernel: usize) -> usize {
        signal - kernel + 1
    }

    #[cfg(test)]
    mod tests {
        pub use super::*;
        #[test]
        pub fn same_samples_and_kernel_same_length() {
            let samples = vec![-1, -1, 0, 1, 1];
            let kernel = vec![-1, -1, 0, 1, 1];
            let mut output = [0; 5];

            same(&samples, &kernel, &mut output);

            assert_eq!(output, [-1, 2, 4, 2, -1])
        }

        #[test]
        pub fn same_samples_5_kernel_even_6() {
            let samples = vec![-1, -1, 0, 1, 1];
            let kernel = vec![-1, -1, -1, 1, 1, 1];
            let mut output = [0; 5];

            same(&samples, &kernel, &mut output);

            assert_eq!(output, [-2, 1, 4, 4, 1])
        }

        #[test]
        pub fn same_samples_longer_than_kernel() {
            let samples = vec![-1, -1, 0, 1, 1, 1, 1, 0, -1, -1];
            let kernel = vec![-1, -1, 0, 1, 1];
            let mut output = [0; 10];

            same(&samples, &kernel, &mut output);

            assert_eq!(output, [-1, 2, 4, 3, 1, -1, -3, -4, -2, 1])
        }

        #[test]
        pub fn valid_samples_and_kernel_valid_length() {
            let samples = vec![-1, -1, 0, 1, 1];
            let kernel = vec![-1, -1, 0, 1, 1];
            let mut output = [0; 1];

            valid(&samples, &kernel, &mut output).unwrap();

            assert_eq!(output, [4])
        }

        #[test]
        pub fn valid_samples_6_kernel_5() {
            let samples = vec![-1, -1, -1, 1, 1, 1];
            let kernel = vec![-1, -1, 0, 1, 1];
            let mut output = [0; 2];

            valid(&samples, &kernel, &mut output).unwrap();

            assert_eq!(output, [4, 4])
        }

        #[test]
        pub fn valid_samples_longer_than_kernel() {
            let samples = vec![-1, -1, 0, 1, 1, 1, 1, 0, -1, -1];
            let kernel = vec![-1, -1, 0, 1, 1];
            let mut output = [0; 6];

            valid(&samples, &kernel, &mut output).unwrap();

            assert_eq!(output, [4, 3, 1, -1, -3, -4])
        }
    }
}

pub fn median_non_averaged<T>(input: &[T]) -> Result<T, ()>
where
    T: PartialOrd + Clone,
{
    let mut ordered = Vec::new();
    ordered.extend(input.iter());
    ordered.sort_by(|lhs, rhs| lhs.partial_cmp(rhs).unwrap());
    if ordered.is_empty() {
        Err(())
    } else {
        Ok(ordered[ordered.len() / 2].clone())
    }
}

pub fn nms<T>(input: &[T]) -> bool
where
    T: PartialOrd + Clone,
{
    let sum = input
        .windows(2)
        .map(|window| match window[0].le(&window[1]) {
            true => 1,
            _ => -1,
        })
        .sum::<isize>();

    sum == 0
}

pub fn begin_upper_limit_slice<'a, T>(input: &'a [T], size: usize) -> &'a [T] {
    let len = input.len();
    &input[..std::cmp::min(size, len)]
}

pub struct WindowedWeightedAverage<T> {
    value: T,
    internal_weight: T,
}

impl<T> WindowedWeightedAverage<T> {
    pub fn new(initial_value: T, internal_weight: T) -> Self {
        Self {
            value: initial_value,
            internal_weight: internal_weight,
        }
    }
    pub fn value(&self) -> &T {
        &self.value
    }
}

impl<T> WindowedWeightedAverage<T>
where
    T: std::ops::Add<T, Output = T>
        + std::ops::Mul<T, Output = T>
        + std::ops::Div<T, Output = T>
        + Clone,
{
    pub fn acc(&mut self, value: T, weight: T) {
        self.value = (self.value.clone() * self.internal_weight.clone() + value * weight.clone())
            / (self.internal_weight.clone() + weight.clone())
    }
}

pub struct BitVec {
    s: Vec<u8>,
    bl: usize,
}

impl BitVec {
    pub fn new() -> Self {
        Self {
            s: Vec::new(),
            bl: 0,
        }
    }

    pub fn push(&mut self, value: bool) {
        self.s.resize(self.bl / 8 + 1, 0u8);
        self.bl += 1;
        *self.s.last_mut().unwrap() =
            Self::set_bit(self.s.last().unwrap().clone(), (self.bl - 1) % 8, value);
    }

    pub fn len(&self) -> usize {
        self.bl
    }

    pub fn read_bit(b: u8, n: u8) -> bool {
        b & (0b_1_u8 << (7 - n)) != 0u8
    }

    pub fn truncate_last_incomplete_byte(&mut self) {
        if self.bl % 8 > 0 {
            self.s.truncate(self.s.len() - 1);
            self.bl = (self.bl / 8) * 8;
        }
    }

    pub fn set_bit(b: u8, n: usize, value: bool) -> u8 {
        let bit = 0b_1_u8 << (7 - n);
        let result = b & !bit;
        if value {
            result | bit
        } else {
            result
        }
    }

    pub fn byte_vec(&self) -> &Vec<u8> {
        &self.s
    }
}
