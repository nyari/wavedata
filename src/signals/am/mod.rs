#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Transition {
    Rising,
    Falling,
    Hold(usize),
    Noise(usize),
}
