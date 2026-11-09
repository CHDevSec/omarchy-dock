//! Animation primitives.

/// Damped harmonic oscillator, integrated semi-implicitly.
///
/// Preferred over fixed-duration easing because it is interruptible: when the
/// pointer leaves early the icon reverses from wherever it currently is,
/// carrying its velocity, instead of snapping or restarting a tween.
#[derive(Clone, Copy, Debug)]
pub struct Spring {
    pub pos: f64,
    pub vel: f64,
    pub target: f64,
}

/// Whether springs animate. Off, `step` lands on the target at once, so every
/// motion in the dock becomes instant without each caller checking.
static ENABLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_enabled(on: bool) {
    ENABLED.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// Longest frame the integrator will accept. A compositor stall or a laptop
/// resume would otherwise produce a huge `dt` and blow the spring up.
const MAX_DT: f64 = 1.0 / 30.0;

/// Longest single integration step.
const SUBSTEP: f64 = 1.0 / 480.0;

impl Spring {
    pub fn at(v: f64) -> Self {
        Self { pos: v, vel: 0.0, target: v }
    }

    pub fn step(&mut self, dt: f64, stiffness: f64, damping: f64) {
        if !ENABLED.load(std::sync::atomic::Ordering::Relaxed) {
            self.settle();
            return;
        }
        let dt = dt.clamp(0.0, MAX_DT);
        // Semi-implicit Euler is only stable while `damping * h` and
        // `sqrt(stiffness) * h` stay well under 2. A stiff, critically damped
        // spring on a slow frame breaks that and blows up, so integrate in
        // substeps short enough for any spring the dock uses.
        let steps = (dt / SUBSTEP).ceil().max(1.0);
        let h = dt / steps;
        for _ in 0..steps as usize {
            let accel = stiffness * (self.target - self.pos) - damping * self.vel;
            self.vel += accel * h;
            self.pos += self.vel * h;
        }
    }

    pub fn settled(&self) -> bool {
        (self.target - self.pos).abs() < 0.0005 && self.vel.abs() < 0.0005
    }

    /// Snap exactly onto the target so a settled item renders identically each
    /// frame and the tick loop can safely stop.
    pub fn settle(&mut self) {
        self.pos = self.target;
        self.vel = 0.0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stiff_critically_damped_spring_settles_on_slow_frames() {
        // The wave's tracking spring on a 30fps frame: plain Euler diverges.
        let k = 2400.0;
        let mut s = Spring::at(1.0);
        s.target = 1.6;
        for _ in 0..60 {
            s.step(1.0 / 30.0, k, 2.0 * f64::sqrt(k));
            assert!(s.pos.is_finite() && s.pos <= 1.6 + 1e-6, "no overshoot: {}", s.pos);
        }
        assert!(s.settled());
    }
}
