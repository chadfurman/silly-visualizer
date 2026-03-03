use crate::genome::Genome;

const PROFILE_SIZE: usize = 5;

pub struct ChangeDetector {
    baseline: [f32; PROFILE_SIZE],
    current: [f32; PROFILE_SIZE],
    threshold: f32,
    cooldown: f32,
    time_since_trigger: f32,
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

    pub fn update(&mut self, profile: &[f32; PROFILE_SIZE], dt: f32) -> bool {
        self.update_emas(profile, dt);
        self.time_since_trigger += dt;
        if !self.primed {
            return self.try_prime();
        }
        self.check_trigger()
    }

    fn update_emas(&mut self, profile: &[f32; PROFILE_SIZE], dt: f32) {
        let slow_alpha = (dt / 10.0).clamp(0.0, 1.0);
        let fast_alpha = (dt / 2.0).clamp(0.0, 1.0);
        for (i, &val) in profile.iter().enumerate() {
            self.baseline[i] += (val - self.baseline[i]) * slow_alpha;
            self.current[i] += (val - self.current[i]) * fast_alpha;
        }
    }

    fn try_prime(&mut self) -> bool {
        if self.time_since_trigger > self.cooldown {
            self.primed = true;
        }
        false
    }

    fn check_trigger(&mut self) -> bool {
        let trigger = self.novelty() > self.threshold
            && self.time_since_trigger >= self.cooldown;
        if trigger {
            self.time_since_trigger = 0.0;
        }
        trigger
    }

    pub fn novelty(&self) -> f32 {
        let mut sum_sq = 0.0f32;
        for i in 0..PROFILE_SIZE {
            let diff = self.current[i] - self.baseline[i];
            sum_sq += diff * diff;
        }
        sum_sq.sqrt()
    }

    #[allow(dead_code)]
    pub fn reset(&mut self) {
        self.baseline = [0.0; PROFILE_SIZE];
        self.current = [0.0; PROFILE_SIZE];
        self.time_since_trigger = 0.0;
        self.primed = false;
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum CrossfadeMode {
    ParamInterpolation,
    FeedbackMelt,
    Both,
}

impl CrossfadeMode {
    pub fn from_genome_value(v: f32) -> Self {
        match v as u32 {
            0 => CrossfadeMode::ParamInterpolation,
            1 => CrossfadeMode::FeedbackMelt,
            _ => CrossfadeMode::Both,
        }
    }

    pub fn duration(&self) -> f32 {
        match self {
            CrossfadeMode::ParamInterpolation => 4.0,
            CrossfadeMode::FeedbackMelt => 2.5,
            CrossfadeMode::Both => 4.0,
        }
    }
}

pub struct Crossfade {
    #[allow(dead_code)]
    pub mode: CrossfadeMode,
    pub from: Genome,
    pub to: Genome,
    pub progress: f32,
    pub duration: f32,
}

impl Crossfade {
    pub fn new(from: Genome, to: Genome, mode: CrossfadeMode) -> Self {
        let duration = mode.duration();
        Self { mode, from, to, progress: 0.0, duration }
    }

    pub fn advance(&mut self, dt: f32) -> bool {
        self.progress += dt / self.duration;
        if self.progress >= 1.0 {
            self.progress = 1.0;
            return true;
        }
        false
    }

    pub fn current_genome(&self) -> Genome {
        self.from.lerp(&self.to, self.progress)
    }

    #[allow(dead_code)]
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
        let profile = [0.5; 5];
        assert!(!det.update(&profile, 0.016));
        assert!(!det.update(&profile, 0.016));
    }

    #[test]
    fn change_detector_triggers_on_large_shift() {
        let mut det = ChangeDetector::new(0.05, 1.0);
        let steady = [0.2, 0.2, 0.2, 0.2, 0.0];
        for _ in 0..200 {
            det.update(&steady, 0.016);
        }
        let shifted = [0.9, 0.9, 0.9, 0.9, 1.0];
        let mut triggered = false;
        for _ in 0..100 {
            if det.update(&shifted, 0.016) {
                triggered = true;
                break;
            }
        }
        assert!(triggered);
    }

    #[test]
    fn change_detector_respects_cooldown() {
        let mut det = ChangeDetector::new(0.05, 5.0);
        let steady = [0.2; 5];
        for _ in 0..500 {
            det.update(&steady, 0.016);
        }
        let shifted = [0.9, 0.9, 0.9, 0.9, 1.0];
        let mut first = false;
        for _ in 0..200 {
            if det.update(&shifted, 0.016) { first = true; break; }
        }
        assert!(first);
        let shifted2 = [0.1; 5];
        let mut second = false;
        for _ in 0..10 {
            if det.update(&shifted2, 0.016) { second = true; break; }
        }
        assert!(!second);
    }

    #[test]
    fn change_detector_does_not_trigger_on_steady_signal() {
        let mut det = ChangeDetector::new(0.5, 1.0);
        let steady = [0.3; 5];
        let mut triggered = false;
        for _ in 0..2000 {
            if det.update(&steady, 0.016) { triggered = true; break; }
        }
        assert!(!triggered);
    }

    #[test]
    fn novelty_is_zero_when_equal() {
        let det = ChangeDetector::new(0.1, 1.0);
        assert_eq!(det.novelty(), 0.0);
    }

    #[test]
    fn novelty_converges_on_steady_input() {
        let mut det = ChangeDetector::new(0.1, 1.0);
        let signal = [0.3; 5];
        for _ in 0..10000 {
            det.update(&signal, 0.1);
        }
        assert!(det.novelty() < 0.001);
    }

    #[test]
    fn reset_clears_state() {
        let mut det = ChangeDetector::new(0.1, 1.0);
        let signal = [0.5; 5];
        for _ in 0..100 {
            det.update(&signal, 0.1);
        }
        det.reset();
        assert_eq!(det.novelty(), 0.0);
        assert!(!det.primed);
        assert_eq!(det.time_since_trigger, 0.0);
    }

    #[test]
    fn crossfade_completes_in_expected_time() {
        let mut rng = test_rng();
        let from = Genome::random(&mut rng);
        let to = Genome::random(&mut rng);
        let mut cf = Crossfade::new(from, to, CrossfadeMode::ParamInterpolation);
        let mut steps = 0;
        loop {
            steps += 1;
            if cf.advance(0.1) { break; }
            if steps > 100 { panic!("crossfade did not complete"); }
        }
        assert!((39..=41).contains(&steps));
        assert_eq!(cf.progress, 1.0);
    }

    #[test]
    fn crossfade_genome_at_zero_matches_from() {
        let mut rng = test_rng();
        let from = Genome::random(&mut rng);
        let to = Genome::random(&mut rng);
        let cf = Crossfade::new(from.clone(), to, CrossfadeMode::ParamInterpolation);
        assert_eq!(cf.current_genome(), from);
    }

    #[test]
    fn crossfade_mode_from_genome_value() {
        assert_eq!(CrossfadeMode::from_genome_value(0.0), CrossfadeMode::ParamInterpolation);
        assert_eq!(CrossfadeMode::from_genome_value(1.0), CrossfadeMode::FeedbackMelt);
        assert_eq!(CrossfadeMode::from_genome_value(2.0), CrossfadeMode::Both);
        assert_eq!(CrossfadeMode::from_genome_value(99.0), CrossfadeMode::Both);
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
        let cf = Crossfade::new(from.clone(), to.clone(), CrossfadeMode::ParamInterpolation);
        assert!(!cf.boost_feedback());
        let cf = Crossfade::new(from.clone(), to.clone(), CrossfadeMode::FeedbackMelt);
        assert!(cf.boost_feedback());
        let cf = Crossfade::new(from, to, CrossfadeMode::Both);
        assert!(cf.boost_feedback());
    }

    #[test]
    fn crossfade_advance_returns_false_before_done() {
        let mut rng = test_rng();
        let from = Genome::random(&mut rng);
        let to = Genome::random(&mut rng);
        let mut cf = Crossfade::new(from, to, CrossfadeMode::ParamInterpolation);
        assert!(!cf.advance(0.1));
        assert!(cf.progress > 0.0);
        assert!(cf.progress < 1.0);
    }
}
