use num::Zero;

use crate::sampling::SampleCount;

/// Time offset of a signal in terms of seconds
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub struct Time(f32);

impl Time {
    pub fn new(value: f32) -> Self {
        Self(value)
    }
    pub fn zero() -> Self {
        Self(0f32)
    }
    pub fn value(self) -> f32 {
        self.0
    }
    pub fn frequency(self) -> Frequency {
        Frequency(self.0.recip())
    }
    pub fn mul(self, value: f32) -> Time {
        Time(self.0 * value)
    }
}

impl std::ops::Add for Time {
    type Output = Time;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::AddAssign for Time {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

impl std::ops::Sub for Time {
    type Output = Time;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl std::ops::SubAssign for Time {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl std::ops::Div for Time {
    type Output = f32;

    fn div(self, rhs: Self) -> Self::Output {
        self.0 / rhs.0
    }
}

impl std::ops::Mul<Frequency> for Time {
    type Output = f32;

    fn mul(self, rhs: Frequency) -> Self::Output {
        self.0 * rhs.0
    }
}

impl std::ops::Mul<Proportion> for Time {
    type Output = Time;
    fn mul(self, rhs: Proportion) -> Self::Output {
        Self(self.0 * rhs.value())
    }
}

/// Frequency of a signal in terms of 1/s a.k.a Hz
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub struct Frequency(f32);

impl Frequency {
    pub fn new(value: f32) -> Self {
        Self(value)
    }
    pub fn value(self) -> f32 {
        self.0
    }
    pub fn cycle_time(self) -> Time {
        Time(self.0.recip())
    }
}

impl std::ops::Div for Frequency {
    type Output = f32;
    fn div(self, rhs: Self) -> Self::Output {
        self.0 / rhs.0
    }
}

impl std::ops::Div<SampleCount> for Frequency {
    type Output = Frequency;
    fn div(self, rhs: SampleCount) -> Self::Output {
        Self(self.0 / (rhs.value() as f32))
    }
}

/// Maximum amplitude of a signal
#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub struct Amplitude(f32);

impl Amplitude {
    pub fn new(value: f32) -> Self {
        Self(value)
    }
    pub fn value(self) -> f32 {
        self.0
    }
    pub fn zero() -> Self {
        Self(0.0)
    }
    pub fn relative_to(self, rhs: Self) -> Proportion {
        let denominator = if rhs.0.is_zero() { f32::EPSILON } else { rhs.0 };
        Proportion(self.0 / denominator)
    }
    pub fn mul(self, value: f32) -> Amplitude {
        Amplitude(self.0 * value)
    }
    pub fn div(self, value: f32) -> Amplitude {
        Amplitude(self.0 / value)
    }
    pub fn abs(self) -> Amplitude {
        Amplitude(self.0.abs())
    }
}

impl num::traits::Zero for Amplitude {
    fn zero() -> Self {
        Amplitude(0.0)
    }

    fn is_zero(&self) -> bool {
        (Self::zero() <= *self) && (*self <= Self::zero())
    }
}

impl std::ops::Add for Amplitude {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Mul for Amplitude {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        Self(self.0 * rhs.0)
    }
}

impl std::ops::Sub for Amplitude {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub struct Proportion(f32);

impl Proportion {
    pub fn new(value: f32) -> Self {
        Self(value)
    }
    pub fn value(self) -> f32 {
        self.0
    }
    pub fn zero() -> Self {
        Self(0.0f32)
    }
    pub fn scale_usize(self, rhs: usize) -> usize {
        (self.0 * (rhs as f32)) as usize
    }
}
trait Clampable<T> {
    fn clamp(self, lower: T, higher: T) -> T;
}

impl Clampable<f32> for f32 {
    fn clamp(self, lower: f32, higher: f32) -> f32 {
        self.max(lower).min(higher)
    }
}
