use crate::units::{Amplitude, Frequency, Time};

pub trait Wave: Sized + Send {
    fn shift_mut(&mut self, offset: Time);
    fn value_at(&self, t: Time) -> Amplitude;

    fn shift(mut self, offset: Time) -> Self {
        self.shift_mut(offset);
        self
    }
}

pub struct Sine {
    freq: Frequency,
    phase_offset: Time,
    amplitude: Amplitude,
}

impl Sine {
    pub fn new(freq: Frequency, phase_offset: Time, amplitude: Amplitude) -> Self {
        Self {
            freq: freq,
            phase_offset: phase_offset,
            amplitude: amplitude,
        }
    }
}

impl Wave for Sine {
    fn shift_mut(&mut self, offset: Time) {
        let new_phase_offset_base = self.phase_offset + offset;
        let cycle_time = self.freq.cycle_time();
        let whole_phases = (new_phase_offset_base / cycle_time).floor();
        self.phase_offset = new_phase_offset_base - (cycle_time * whole_phases);
    }

    fn value_at(&self, t: Time) -> Amplitude {
        let offset_t = self.phase_offset + t;
        let apply_pi = offset_t * 2.0f32 * std::f32::consts::PI;
        let apply_frequency = apply_pi * self.freq;
        Amplitude::new(apply_frequency.sin() * self.amplitude.value())
    }
}

impl<T: Wave> crate::signals::Signal for T {
    fn advance_with(&mut self, dt: Time) -> Result<Amplitude, crate::signals::Error> {
        let result = self.value_at(Time::zero());
        self.shift_mut(dt);
        Ok(result)
    }
}
