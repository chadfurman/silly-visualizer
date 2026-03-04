use rand::Rng;

use crate::genome::Genome;

const CHILD_WEIGHT: f32 = 1.0;

const ANCESTOR_WEIGHTS: [f32; 3] = [0.5, 0.25, 0.125];

pub struct Lineage {
    pub child: Genome,
    pub parent: Option<Genome>,
    pub grandparent: Option<Genome>,
    pub great_grandparent: Option<Genome>,
}

impl Lineage {
    pub fn new(initial: Genome) -> Self {
        Self {
            child: initial,
            parent: None,
            grandparent: None,
            great_grandparent: None,
        }
    }

    pub fn generation_count(&self) -> usize {
        1 + self.parent.is_some() as usize
            + self.grandparent.is_some() as usize
            + self.great_grandparent.is_some() as usize
    }

    pub fn advance(&mut self, rng: &mut impl Rng, mutation_rate: f32) {
        let new_child = self.child.mutate(rng, mutation_rate);
        let blended = self.apply_ancestral_pull(new_child);
        self.shift_generations(blended);
    }

    pub fn inject(&mut self, genome: Genome) {
        self.shift_generations(genome);
    }

    fn ancestors(&self) -> [Option<&Genome>; 3] {
        [
            self.parent.as_ref(),
            self.grandparent.as_ref(),
            self.great_grandparent.as_ref(),
        ]
    }

    fn apply_ancestral_pull(&self, new_child: Genome) -> Genome {
        let mut blended = new_child;
        let mut total_weight = CHILD_WEIGHT;
        for (ancestor, &weight) in self.ancestors().iter().zip(&ANCESTOR_WEIGHTS) {
            let Some(ancestor) = ancestor else { break };
            blended = blended.lerp(ancestor, weight / (total_weight + weight));
            total_weight += weight;
        }
        blended
    }

    /// Advance by picking a random preset and applying light mutation.
    /// The crossfade system handles smooth transitions from current → new.
    #[allow(dead_code)] // Will be called from app.rs in Task 3
    pub fn advance_from_preset(&mut self, rng: &mut impl Rng) {
        let new_child = crate::presets::random_preset_mutated(rng);
        self.shift_generations(new_child);
    }

    fn shift_generations(&mut self, new_child: Genome) {
        self.great_grandparent = self.grandparent.take();
        self.grandparent = self.parent.take();
        self.parent = Some(self.child.clone());
        self.child = new_child;
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
    fn new_lineage_has_one_generation() {
        let mut rng = test_rng();
        let lineage = Lineage::new(Genome::random(&mut rng));
        assert_eq!(lineage.generation_count(), 1);
        assert!(lineage.parent.is_none());
        assert!(lineage.grandparent.is_none());
        assert!(lineage.great_grandparent.is_none());
    }

    #[test]
    fn advance_shifts_generations() {
        let mut rng = test_rng();
        let g = Genome::random(&mut rng);
        let original = g.clone();
        let mut lineage = Lineage::new(g);
        lineage.advance(&mut rng, 0.5);
        assert_eq!(lineage.generation_count(), 2);
        assert_eq!(lineage.parent.as_ref().unwrap(), &original);
    }

    #[test]
    fn advance_caps_at_four_generations() {
        let mut rng = test_rng();
        let mut lineage = Lineage::new(Genome::random(&mut rng));
        for _ in 0..10 {
            lineage.advance(&mut rng, 0.5);
        }
        assert_eq!(lineage.generation_count(), 4);
    }

    #[test]
    fn great_grandparent_replaced_on_fifth_advance() {
        let mut rng = test_rng();
        let mut lineage = Lineage::new(Genome::random(&mut rng));
        for _ in 0..3 {
            lineage.advance(&mut rng, 0.5);
        }
        assert_eq!(lineage.generation_count(), 4);
        let old_gg = lineage.great_grandparent.clone().unwrap();
        lineage.advance(&mut rng, 0.5);
        assert_ne!(lineage.great_grandparent.as_ref().unwrap(), &old_gg);
    }

    #[test]
    fn child_is_always_valid_after_advance() {
        let mut rng = test_rng();
        let mut lineage = Lineage::new(Genome::random(&mut rng));
        for _ in 0..20 {
            lineage.advance(&mut rng, 0.8);
            assert!(lineage.child.is_valid());
        }
    }

    #[test]
    fn inject_shifts_generations() {
        let mut rng = test_rng();
        let g1 = Genome::random(&mut rng);
        let g2 = Genome::random(&mut rng);
        let original = g1.clone();
        let mut lineage = Lineage::new(g1);
        lineage.inject(g2.clone());
        assert_eq!(lineage.child, g2);
        assert_eq!(lineage.parent.as_ref().unwrap(), &original);
    }

    #[test]
    fn ancestral_influence_creates_family_resemblance() {
        let mut rng = test_rng();
        let g = Genome::random(&mut rng);
        let original = g.clone();
        let mut lineage = Lineage::new(g);
        for _ in 0..5 {
            lineage.advance(&mut rng, 0.1);
        }
        let diff = (lineage.child.cam_distance - original.cam_distance).abs();
        assert!(diff < 4.0, "cam_distance drifted too far: {diff}");
    }

    #[test]
    fn ancestors_returns_correct_count() {
        let mut rng = test_rng();
        let mut lineage = Lineage::new(Genome::random(&mut rng));
        assert!(lineage.ancestors().iter().all(|a| a.is_none()));
        lineage.advance(&mut rng, 0.5);
        assert!(lineage.ancestors()[0].is_some());
        assert!(lineage.ancestors()[1].is_none());
    }

    #[test]
    fn apply_ancestral_pull_with_no_ancestors_returns_same() {
        let mut rng = test_rng();
        let lineage = Lineage::new(Genome::random(&mut rng));
        let child = Genome::random(&mut rng);
        let pulled = lineage.apply_ancestral_pull(child.clone());
        assert_eq!(pulled, child);
    }

    #[test]
    fn advance_from_preset_produces_valid_genome() {
        let mut rng = test_rng();
        let initial = crate::presets::random_preset_mutated(&mut rng);
        let mut lineage = Lineage::new(initial);
        for _ in 0..20 {
            lineage.advance_from_preset(&mut rng);
            assert!(lineage.child.is_valid());
        }
    }

    #[test]
    fn advance_from_preset_changes_child() {
        let mut rng = test_rng();
        let initial = crate::presets::random_preset_mutated(&mut rng);
        let mut lineage = Lineage::new(initial);
        let original = lineage.child.clone();
        lineage.advance_from_preset(&mut rng);
        assert_ne!(lineage.child, original);
    }
}
