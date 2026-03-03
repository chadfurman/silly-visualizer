use crate::genome::Genome;

/// Number of spectral profile channels (bass, mids, highs, energy, beat).
const PROFILE_SIZE: usize = 5;

/// Detects significant changes in musical "feel" by comparing a fast-moving
/// EMA (current) against a slow-moving EMA (baseline).
pub struct ChangeDetector {
    /// Slow-moving exponential moving average (alpha = dt/10.0).
    baseline: [f32; PROFILE_SIZE],
    /// Fast-moving exponential moving average (alpha = dt/2.0).
    current: [f32; PROFILE_SIZE],
    /// Novelty threshold to trigger a scene change.
    threshold: f32,
    /// Minimum time between triggers (seconds).
    cooldown: f32,
    /// Time elapsed since last trigger.
    time_since_trigger: f32,
    /// Whether the detector is primed (has enough data to trigger).
    primed: bool,
}

impl ChangeDetector {
    pub fn new(threshold: f32, cooldown: f32) -> Self {
        Self {
            baseline: [0.0; PROFILE_SIZE],
            current: [0.0; PROFILE_SIZE],
            threshold,
            cooldown,
            time_since_trigger: 0.0,
            primed: false,
        }
    }

    /// Update with a new spectral profile. Returns `true` if a significant
    /// musical change was detected.
    pub fn update(&mut self, profile: &[f32; PROFILE_SIZE], dt: f32) -> bool {
        let slow_alpha = (dt / 10.0).clamp(0.0, 1.0);
        let fast_alpha = (dt / 2.0).clamp(0.0, 1.0);

        for i in 0..PROFILE_SIZE {
            self.baseline[i] += (profile[i] - self.baseline[i]) * slow_alpha;
            self.current[i] += (profile[i] - self.current[i]) * fast_alpha;
        }

        self.time_since_trigger += dt;

        // Need some warmup time before we can detect changes
        if !self.primed {
            if self.time_since_trigger > self.cooldown {
                self.primed = true;
            }
            return false;
        }

        let novelty = self.novelty();
        if novelty > self.threshold && self.time_since_trigger >= self.cooldown {
            self.time_since_trigger = 0.0;
            return true;
        }

        false
    }

    /// Euclidean distance between current (fast EMA) and baseline (slow EMA).
    pub fn novelty(&self) -> f32 {
        let mut sum_sq = 0.0f32;
        for i in 0..PROFILE_SIZE {
            let diff = self.current[i] - self.baseline[i];
            sum_sq += diff * diff;
        }
        sum_sq.sqrt()
    }

    /// Reset the detector state.
    pub fn reset(&mut self) {
        self.baseline = [0.0; PROFILE_SIZE];
        self.current = [0.0; PROFILE_SIZE];
        self.time_since_trigger = 0.0;
        self.primed = false;
    }
}

/// Crossfade transition mode between scenes.
#[derive(Clone, Debug, PartialEq)]
pub enum CrossfadeMode {
    /// Pure parameter interpolation (4 seconds).
    ParamInterpolation,
    /// Feedback-based melt effect (2.5 seconds).
    FeedbackMelt,
    /// Both interpolation and melt (4 seconds).
    Both,
}

impl CrossfadeMode {
    /// Map a genome's transition_type f32 value to a mode.
    pub fn from_genome_value(v: f32) -> Self {
        match v as u32 {
            0 => CrossfadeMode::ParamInterpolation,
            1 => CrossfadeMode::FeedbackMelt,
            _ => CrossfadeMode::Both,
        }
    }

    /// Duration of the crossfade in seconds.
    pub fn duration(&self) -> f32 {
        match self {
            CrossfadeMode::ParamInterpolation => 4.0,
            CrossfadeMode::FeedbackMelt => 2.5,
            CrossfadeMode::Both => 4.0,
        }
    }
}

/// Manages an active crossfade between two genomes.
pub struct Crossfade {
    pub mode: CrossfadeMode,
    pub from: Genome,
    pub to: Genome,
    pub progress: f32,
    pub duration: f32,
}

impl Crossfade {
    /// Create a new crossfade transition.
    pub fn new(from: Genome, to: Genome, mode: CrossfadeMode) -> Self {
        let duration = mode.duration();
        Self {
            mode,
            from,
            to,
            progress: 0.0,
            duration,
        }
    }

    /// Advance the crossfade by `dt` seconds. Returns `true` when complete.
    pub fn advance(&mut self, dt: f32) -> bool {
        self.progress += dt / self.duration;
        if self.progress >= 1.0 {
            self.progress = 1.0;
            true
        } else {
            false
        }
    }

    /// Get the interpolated genome at the current progress.
    pub fn current_genome(&self) -> Genome {
        self.from.lerp(&self.to, self.progress)
    }

    /// Whether the feedback/melt effect should be boosted during this crossfade.
    pub fn boost_feedback(&self) -> bool {
        matches!(self.mode, CrossfadeMode::FeedbackMelt | CrossfadeMode::Both)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;

    fn test_rng() -> SmallRng {
        SmallRng::seed_from_u64(42)
    }

    #[test]
    fn change_detector_does_not_trigger_initially() {
        let mut det = ChangeDetector::new(0.1, 2.0);
        let profile = [0.5, 0.5, 0.5, 0.5, 0.5];
        // Even with data, should not trigger until primed (after cooldown)
        assert!(!det.update(&profile, 0.016));
        assert!(!det.update(&profile, 0.016));
    }

    #[test]
    fn change_detector_triggers_on_large_shift() {
        let mut det = ChangeDetector::new(0.05, 1.0);

        // Feed steady signal to prime
        let steady = [0.2, 0.2, 0.2, 0.2, 0.0];
        for _ in 0..200 {
            det.update(&steady, 0.016);
        }

        // Now shift dramatically
        let shifted = [0.9, 0.9, 0.9, 0.9, 1.0];
        // Feed shifted signal enough times for the fast EMA to catch up
        let mut triggered = false;
        for _ in 0..100 {
            if det.update(&shifted, 0.016) {
                triggered = true;
                break;
            }
        }
        assert!(triggered, "should trigger on large spectral shift");
    }

    #[test]
    fn change_detector_respects_cooldown() {
        let mut det = ChangeDetector::new(0.05, 5.0);

        // Prime
        let steady = [0.2, 0.2, 0.2, 0.2, 0.0];
        for _ in 0..500 {
            det.update(&steady, 0.016);
        }

        // Trigger once
        let shifted = [0.9, 0.9, 0.9, 0.9, 1.0];
        let mut first_trigger = false;
        for _ in 0..200 {
            if det.update(&shifted, 0.016) {
                first_trigger = true;
                break;
            }
        }
        assert!(first_trigger, "first trigger should happen");

        // Immediately try another shift — should be blocked by cooldown
        let shifted2 = [0.1, 0.1, 0.1, 0.1, 0.0];
        // Only advance a tiny bit (well within 5s cooldown)
        let mut second_trigger = false;
        for _ in 0..10 {
            if det.update(&shifted2, 0.016) {
                second_trigger = true;
                break;
            }
        }
        assert!(
            !second_trigger,
            "second trigger should be blocked by cooldown"
        );
    }

    #[test]
    fn change_detector_does_not_trigger_on_steady_signal() {
        // Use a higher threshold so the initial ramp-up from [0;5] to steady
        // doesn't exceed it. The key insight: a truly steady signal should
        // converge both EMAs and never trigger after warmup.
        let mut det = ChangeDetector::new(0.5, 1.0);
        let steady = [0.3, 0.3, 0.3, 0.3, 0.3];

        let mut triggered = false;
        for _ in 0..2000 {
            if det.update(&steady, 0.016) {
                triggered = true;
                break;
            }
        }
        assert!(
            !triggered,
            "should not trigger on perfectly steady signal"
        );
    }

    #[test]
    fn novelty_is_zero_when_equal() {
        let mut det = ChangeDetector::new(0.1, 1.0);
        // Both baseline and current start at zero
        assert_eq!(det.novelty(), 0.0);

        // Feed a steady signal for a long time — both EMAs should converge
        let signal = [0.3, 0.3, 0.3, 0.3, 0.3];
        for _ in 0..10000 {
            det.update(&signal, 0.1);
        }
        // After convergence, novelty should be very close to zero
        assert!(
            det.novelty() < 0.001,
            "novelty should be ~0 after convergence, got {}",
            det.novelty()
        );
    }

    #[test]
    fn crossfade_completes_in_expected_time() {
        let mut rng = test_rng();
        let from = Genome::random(&mut rng);
        let to = Genome::random(&mut rng);
        let mut cf = Crossfade::new(from, to, CrossfadeMode::ParamInterpolation);

        // 4s duration, advance in 0.1s steps. Due to floating point
        // accumulation, expect ~40-41 steps.
        let mut steps = 0;
        loop {
            steps += 1;
            if cf.advance(0.1) {
                break;
            }
            if steps > 100 {
                panic!("crossfade did not complete");
            }
        }
        // Should take approximately 40 steps (4.0 / 0.1), allow +-1 for float accumulation
        assert!(
            (39..=41).contains(&steps),
            "should complete in ~40 steps, got {steps}"
        );
        assert_eq!(cf.progress, 1.0);
    }

    #[test]
    fn crossfade_genome_at_zero_matches_from() {
        let mut rng = test_rng();
        let from = Genome::random(&mut rng);
        let to = Genome::random(&mut rng);
        let cf = Crossfade::new(from.clone(), to, CrossfadeMode::ParamInterpolation);

        // progress is 0.0 initially
        let current = cf.current_genome();
        assert_eq!(current, from);
    }

    #[test]
    fn crossfade_mode_from_genome_value() {
        assert_eq!(
            CrossfadeMode::from_genome_value(0.0),
            CrossfadeMode::ParamInterpolation
        );
        assert_eq!(
            CrossfadeMode::from_genome_value(1.0),
            CrossfadeMode::FeedbackMelt
        );
        assert_eq!(
            CrossfadeMode::from_genome_value(2.0),
            CrossfadeMode::Both
        );
    }

    #[test]
    fn crossfade_melt_duration() {
        assert_eq!(CrossfadeMode::FeedbackMelt.duration(), 2.5);
        assert_eq!(CrossfadeMode::ParamInterpolation.duration(), 4.0);
        assert_eq!(CrossfadeMode::Both.duration(), 4.0);
    }

    #[test]
    fn crossfade_boost_feedback() {
        let mut rng = test_rng();
        let from = Genome::random(&mut rng);
        let to = Genome::random(&mut rng);

        let cf_interp =
            Crossfade::new(from.clone(), to.clone(), CrossfadeMode::ParamInterpolation);
        assert!(!cf_interp.boost_feedback());

        let cf_melt = Crossfade::new(from.clone(), to.clone(), CrossfadeMode::FeedbackMelt);
        assert!(cf_melt.boost_feedback());

        let cf_both = Crossfade::new(from, to, CrossfadeMode::Both);
        assert!(cf_both.boost_feedback());
    }
}
