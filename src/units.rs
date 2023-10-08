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

impl<T> std::ops::Mul<T> for Time
where
    T: std::ops::Mul<f32, Output = f32>,
{
    type Output = Time;

    fn mul(self, rhs: T) -> Self::Output {
        Self(rhs * self.0)
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
}

impl std::ops::Add for Amplitude {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl std::ops::Sub for Amplitude {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl<T> std::ops::Mul<T> for Amplitude
where
    T: std::ops::Mul<f32, Output = f32>,
{
    type Output = Amplitude;
    fn mul(self, rhs: T) -> Self::Output {
        Self(rhs * self.0)
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
}

trait Clampable<T> {
    fn clamp(self, lower: T, higher: T) -> T;
}

impl Clampable<f32> for f32 {
    fn clamp(self, lower: f32, higher: f32) -> f32 {
        self.max(lower).min(higher)
    }
}
