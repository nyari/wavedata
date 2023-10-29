pub fn convolve1d<T>(signal: &[T], kernel: &[T], result: &mut [T])
where
    T: std::ops::Add<T, Output = T> + std::ops::Mul<T, Output = T> + num::traits::Zero + Clone,
{
    signal
        .windows(kernel.len())
        .map(|window| {
            window
                .iter()
                .zip(kernel.iter())
                .fold(T::zero(), |acc, (lhs, rhs)| acc + lhs.clone() * rhs.clone())
        })
        .enumerate()
        .for_each(|(idx, value)| result[idx] = value);

    let padding_idx = signal.len() - kernel.len() + 1;
    for signal_idx in padding_idx..signal.len() {
        result[signal_idx] = T::zero();
        for idx in signal_idx..signal.len() {
            let kernel_idx = idx - signal_idx;
            result[signal_idx] =
                result[signal_idx].clone() + (signal[idx].clone() * kernel[kernel_idx].clone());
        }
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
