//! Finite, frame-rate-independent visual motion. Document state never waits for it.
use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug)]
pub struct Motion {
    from: f32,
    pub target: f32,
    started: Instant,
    duration: Duration,
}

impl Motion {
    pub fn new(value: f32, now: Instant, duration: Duration) -> Self {
        Self {
            from: value,
            target: value,
            started: now,
            duration,
        }
    }

    pub fn sample(&self, now: Instant) -> (f32, bool) {
        let elapsed = now.saturating_duration_since(self.started);
        if self.from == self.target || elapsed >= self.duration {
            return (self.target, false);
        }
        let t = elapsed.as_secs_f32() / self.duration.as_secs_f32();
        let eased = 1. - (1. - t).powi(3);
        (self.from + (self.target - self.from) * eased, true)
    }

    pub fn retarget(&mut self, target: f32, now: Instant) {
        // Repainting an unchanged target must not restart its clock.
        if target != self.target {
            self.from = self.sample(now).0;
            self.target = target;
            self.started = now;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn motion_is_finite_and_independent_of_frame_count() {
        let now = Instant::now();
        let mut motion = Motion::new(0., now, Duration::from_millis(120));
        motion.retarget(100., now);
        assert_eq!(motion.sample(now), (0., true));
        assert_eq!(motion.sample(now + Duration::from_millis(60)), (87.5, true));
        motion.retarget(100., now + Duration::from_millis(90));
        assert_eq!(
            motion.sample(now + Duration::from_millis(120)),
            (100., false)
        );
        assert_eq!(motion.sample(now + Duration::from_secs(2)), (100., false));
    }

    #[test]
    fn rapid_retarget_and_reversal_do_not_jump() {
        let now = Instant::now();
        let mut motion = Motion::new(0., now, Duration::from_millis(80));
        motion.retarget(100., now);
        let tick = now + Duration::from_millis(20);
        let before = motion.sample(tick).0;
        motion.retarget(-30., tick);
        assert_eq!(motion.sample(tick).0, before);
        assert!(motion.sample(tick + Duration::from_millis(20)).0 < before);
        assert_eq!(
            motion.sample(tick + Duration::from_millis(80)),
            (-30., false)
        );
    }

    #[test]
    fn zero_duration_and_equal_endpoints_settle_immediately() {
        let now = Instant::now();
        let mut motion = Motion::new(5., now, Duration::ZERO);
        assert_eq!(motion.sample(now), (5., false));
        motion.retarget(15., now);
        assert_eq!(motion.sample(now), (15., false));
    }
}
