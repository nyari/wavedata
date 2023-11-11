use std::collections::VecDeque;

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

#[derive(PartialEq, PartialOrd)]
pub struct BitIndex(usize, u8);

impl BitIndex {
    pub fn new(idx: usize) -> Self {
        Self(idx / 8, (idx % 8) as u8)
    }

    pub fn comp(byte_idx: usize, bit_idx: u8) -> Self {
        Self(byte_idx, bit_idx)
    }

    pub fn len(&self) -> usize {
        self.0 * 8 + Into::<usize>::into(self.1)
    }

    pub fn is_last(&self) -> bool {
        self.1 == 7
    }

    pub fn offset(self, offset: isize) -> Self {
        let byte_idx_offset = offset / 8;
        let bit_idx_offset = (offset % 8) as i8;

        let mut result_byte_idx = self.0 as isize;
        let mut result_bit_idx = self.1 as i8;

        result_byte_idx += byte_idx_offset;
        result_bit_idx += bit_idx_offset;

        if result_bit_idx < 0 {
            result_byte_idx -= 1;
            result_bit_idx += 8;
        } else if result_bit_idx > 7 {
            result_byte_idx += 1;
            result_bit_idx -= 8;
        }

        if result_byte_idx < 0 {
            panic!("Incorrect resulting index");
        }

        Self(result_byte_idx as usize, result_bit_idx as u8)
    }
}

pub struct BitVec(Vec<u8>, u8);

impl BitVec {
    pub fn new() -> Self {
        Self(Vec::new(), 0)
    }

    pub fn push(&mut self, value: bool) {
        let blen = self.blen().offset(1);
        self.0.resize(blen.0, 0u8);
        self.1 = blen.1;
        self.0[blen.0] = Self::set_bit(self.0[blen.0].clone(), blen.1)
    }

    pub fn len(&self) -> usize {
        self.blen().len()
    }

    fn blen(&self) -> BitIndex {
        BitIndex::comp(self.0.len(), self.1)
    }

    pub fn read_bit(b: u8, n: u8) -> bool {
        b & (0b_1_u8 << n) != 0u8
    }

    pub fn set_bit(b: u8, n: u8) -> u8 {
        b | (0b_1_u8 << n)
    }
}
