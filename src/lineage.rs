use rand::Rng;

use crate::genome::Genome;

/// Ancestral weights for blending influence from older generations.
const CHILD_WEIGHT: f32 = 1.0;
const PARENT_WEIGHT: f32 = 0.5;
const GRANDPARENT_WEIGHT: f32 = 0.25;
const GREAT_GRANDPARENT_WEIGHT: f32 = 0.125;

/// Manages a lineage of up to 4 generations of genomes.
///
/// The child is the active genome. When advancing, the child is mutated
/// to create a new child, and all generations shift up (parent becomes
/// grandparent, etc.). The great-grandparent is dropped when a fifth
/// generation would be added.
///
/// Ancestral pull: after mutation, the new child is blended toward
/// ancestors via weighted lerp to maintain family resemblance.
pub struct Lineage {
    pub child: Genome,
    pub parent: Option<Genome>,
    pub grandparent: Option<Genome>,
    pub great_grandparent: Option<Genome>,
}

impl Lineage {
    /// Create a new lineage with a single generation.
    pub fn new(initial: Genome) -> Self {
        Self {
            child: initial,
            parent: None,
            grandparent: None,
            great_grandparent: None,
        }
    }

    /// How many generations exist (1–4).
    pub fn generation_count(&self) -> usize {
        1 + self.parent.is_some() as usize
            + self.grandparent.is_some() as usize
            + self.great_grandparent.is_some() as usize
    }

    /// Mutate the current child to create a new child, shifting all
    /// generations up. Applies ancestral pull via weighted lerp.
    pub fn advance(&mut self, rng: &mut impl Rng, mutation_rate: f32) {
        let mut new_child = self.child.mutate(rng, mutation_rate);

        // Apply ancestral pull: blend new_child toward ancestors
        // using weighted averages. The total weight normalizes the blend.
        let mut total_weight = CHILD_WEIGHT;
        let mut blended = new_child.clone();

        if let Some(ref parent) = self.parent {
            blended = blended.lerp(parent, PARENT_WEIGHT / (total_weight + PARENT_WEIGHT));
            total_weight += PARENT_WEIGHT;
        }
        if let Some(ref grandparent) = self.grandparent {
            blended =
                blended.lerp(grandparent, GRANDPARENT_WEIGHT / (total_weight + GRANDPARENT_WEIGHT));
            total_weight += GRANDPARENT_WEIGHT;
        }
        if let Some(ref great_grandparent) = self.great_grandparent {
            blended = blended.lerp(
                great_grandparent,
                GREAT_GRANDPARENT_WEIGHT / (total_weight + GREAT_GRANDPARENT_WEIGHT),
            );
            let _ = total_weight; // suppress unused warning
        }

        new_child = blended;

        // Shift generations up
        self.great_grandparent = self.grandparent.take();
        self.grandparent = self.parent.take();
        self.parent = Some(self.child.clone());
        self.child = new_child;
    }

    /// Inject a specific genome as the new child, shifting generations up.
    pub fn inject(&mut self, genome: Genome) {
        self.great_grandparent = self.grandparent.take();
        self.grandparent = self.parent.take();
        self.parent = Some(self.child.clone());
        self.child = genome;
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
        let g = Genome::random(&mut rng);
        let lineage = Lineage::new(g);
        assert_eq!(lineage.generation_count(), 1);
        assert!(lineage.parent.is_none());
        assert!(lineage.grandparent.is_none());
        assert!(lineage.great_grandparent.is_none());
    }

    #[test]
    fn advance_shifts_generations() {
        let mut rng = test_rng();
        let g = Genome::random(&mut rng);
        let original_child = g.clone();
        let mut lineage = Lineage::new(g);

        lineage.advance(&mut rng, 0.5);

        assert_eq!(lineage.generation_count(), 2);
        assert!(lineage.parent.is_some());
        assert_eq!(lineage.parent.as_ref().unwrap(), &original_child);
        assert!(lineage.grandparent.is_none());
    }

    #[test]
    fn advance_caps_at_four_generations() {
        let mut rng = test_rng();
        let g = Genome::random(&mut rng);
        let mut lineage = Lineage::new(g);

        for _ in 0..10 {
            lineage.advance(&mut rng, 0.5);
        }

        assert_eq!(lineage.generation_count(), 4);
    }

    #[test]
    fn great_grandparent_dies_on_fifth_advance() {
        let mut rng = test_rng();
        let g = Genome::random(&mut rng);
        let mut lineage = Lineage::new(g);

        // Advance 3 times to fill all 4 slots
        lineage.advance(&mut rng, 0.5);
        lineage.advance(&mut rng, 0.5);
        lineage.advance(&mut rng, 0.5);
        assert_eq!(lineage.generation_count(), 4);

        // Store the current great_grandparent
        let old_great_grandparent = lineage.great_grandparent.clone().unwrap();

        // Advance once more — great_grandparent should be replaced
        lineage.advance(&mut rng, 0.5);
        assert_eq!(lineage.generation_count(), 4);

        // The old great_grandparent should no longer be present
        // (the new great_grandparent was the old grandparent)
        assert_ne!(
            lineage.great_grandparent.as_ref().unwrap(),
            &old_great_grandparent
        );
    }

    #[test]
    fn child_is_always_valid_after_advance() {
        let mut rng = test_rng();
        let g = Genome::random(&mut rng);
        let mut lineage = Lineage::new(g);

        for _ in 0..20 {
            lineage.advance(&mut rng, 0.8);
            assert!(
                lineage.child.is_valid(),
                "child should be valid after advance"
            );
        }
    }

    #[test]
    fn inject_shifts_generations() {
        let mut rng = test_rng();
        let g1 = Genome::random(&mut rng);
        let g2 = Genome::random(&mut rng);
        let original_child = g1.clone();
        let mut lineage = Lineage::new(g1);

        lineage.inject(g2.clone());

        assert_eq!(lineage.generation_count(), 2);
        assert_eq!(lineage.child, g2);
        assert_eq!(lineage.parent.as_ref().unwrap(), &original_child);
    }

    #[test]
    fn ancestral_influence_creates_family_resemblance() {
        let mut rng = test_rng();
        let g = Genome::random(&mut rng);
        let original = g.clone();
        let mut lineage = Lineage::new(g);

        // Advance many times with low mutation rate
        for _ in 0..5 {
            lineage.advance(&mut rng, 0.1);
        }

        // The child should still have some resemblance to the original
        // (not wildly different). Check a continuous parameter.
        // cam_distance range is 3.0–8.0, so range width = 5.0
        let diff = (lineage.child.cam_distance - original.cam_distance).abs();
        let range_width = 5.0; // 8.0 - 3.0

        // With ancestral pull and low mutation, should stay within 80% of range
        assert!(
            diff < range_width * 0.8,
            "cam_distance drifted too far: diff={diff}, range={range_width}"
        );
    }
}
