use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rand::Rng;

use crate::genome::Genome;
use crate::lineage::Lineage;

/// Serializable mirror of Lineage (which doesn't derive Serialize/Deserialize).
#[derive(serde::Serialize, serde::Deserialize)]
struct SerializableLineage {
    child: Genome,
    parent: Option<Genome>,
    grandparent: Option<Genome>,
    great_grandparent: Option<Genome>,
}

impl SerializableLineage {
    fn from_lineage(lineage: &Lineage) -> Self {
        Self {
            child: lineage.child.clone(),
            parent: lineage.parent.clone(),
            grandparent: lineage.grandparent.clone(),
            great_grandparent: lineage.great_grandparent.clone(),
        }
    }

    fn into_lineage(self) -> Lineage {
        Lineage {
            child: self.child,
            parent: self.parent,
            grandparent: self.grandparent,
            great_grandparent: self.great_grandparent,
        }
    }
}

/// Get the app data directory (~/.silly-visualizer/), creating it if needed.
pub fn data_dir() -> Option<PathBuf> {
    let home = dirs::home_dir()?;
    let dir = home.join(".silly-visualizer");
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Get the favorites directory (~/.silly-visualizer/favorites/), creating it if needed.
pub fn favorites_dir() -> Option<PathBuf> {
    let dir = data_dir()?.join("favorites");
    fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Save the current lineage to ~/.silly-visualizer/lineage.json
pub fn save_lineage(lineage: &Lineage) -> Result<(), String> {
    let dir = data_dir().ok_or("could not determine data directory")?;
    let path = dir.join("lineage.json");
    let serializable = SerializableLineage::from_lineage(lineage);
    let json = serde_json::to_string_pretty(&serializable)
        .map_err(|e| format!("failed to serialize lineage: {e}"))?;
    fs::write(&path, json).map_err(|e| format!("failed to write {}: {e}", path.display()))
}

/// Load lineage from ~/.silly-visualizer/lineage.json
pub fn load_lineage() -> Result<Lineage, String> {
    let dir = data_dir().ok_or("could not determine data directory")?;
    let path = dir.join("lineage.json");
    let json =
        fs::read_to_string(&path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let serializable: SerializableLineage =
        serde_json::from_str(&json).map_err(|e| format!("failed to deserialize lineage: {e}"))?;
    Ok(serializable.into_lineage())
}

/// Bookmark a genome to ~/.silly-visualizer/favorites/<timestamp>.json
pub fn save_favorite(genome: &Genome) -> Result<PathBuf, String> {
    let dir = favorites_dir().ok_or("could not determine favorites directory")?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("failed to get timestamp: {e}"))?
        .as_secs();
    let path = dir.join(format!("{timestamp}.json"));
    let json = serde_json::to_string_pretty(genome)
        .map_err(|e| format!("failed to serialize genome: {e}"))?;
    fs::write(&path, &json).map_err(|e| format!("failed to write {}: {e}", path.display()))?;
    Ok(path)
}

/// Load a random favorite genome from ~/.silly-visualizer/favorites/
pub fn load_random_favorite() -> Result<Option<Genome>, String> {
    let dir = favorites_dir().ok_or("could not determine favorites directory")?;
    let entries: Vec<PathBuf> = fs::read_dir(&dir)
        .map_err(|e| format!("failed to read favorites dir: {e}"))?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("json") {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    if entries.is_empty() {
        return Ok(None);
    }

    let mut rng = rand::rng();
    let idx = rng.random_range(0..entries.len());
    let path = &entries[idx];
    let json =
        fs::read_to_string(path).map_err(|e| format!("failed to read {}: {e}", path.display()))?;
    let genome: Genome =
        serde_json::from_str(&json).map_err(|e| format!("failed to deserialize favorite: {e}"))?;
    Ok(Some(genome))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rand::SeedableRng;
    use rand::rngs::SmallRng;
    use std::fs;

    fn test_rng() -> SmallRng {
        SmallRng::seed_from_u64(42)
    }

    /// Create a temporary directory for tests and return its path.
    fn test_data_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "silly-visualizer-test-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn save_lineage_to(dir: &PathBuf, lineage: &Lineage) -> Result<(), String> {
        let path = dir.join("lineage.json");
        let serializable = SerializableLineage::from_lineage(lineage);
        let json = serde_json::to_string_pretty(&serializable)
            .map_err(|e| format!("failed to serialize lineage: {e}"))?;
        fs::write(&path, json).map_err(|e| format!("failed to write {}: {e}", path.display()))
    }

    fn load_lineage_from(dir: &PathBuf) -> Result<Lineage, String> {
        let path = dir.join("lineage.json");
        let json = fs::read_to_string(&path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        let serializable: SerializableLineage = serde_json::from_str(&json)
            .map_err(|e| format!("failed to deserialize lineage: {e}"))?;
        Ok(serializable.into_lineage())
    }

    fn save_favorite_to(dir: &PathBuf, genome: &Genome, name: &str) -> Result<PathBuf, String> {
        let path = dir.join(format!("{name}.json"));
        let json = serde_json::to_string_pretty(genome)
            .map_err(|e| format!("failed to serialize genome: {e}"))?;
        fs::write(&path, &json).map_err(|e| format!("failed to write {}: {e}", path.display()))?;
        Ok(path)
    }

    fn load_random_favorite_from(dir: &PathBuf) -> Result<Option<Genome>, String> {
        let entries: Vec<PathBuf> = fs::read_dir(dir)
            .map_err(|e| format!("failed to read favorites dir: {e}"))?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();

        if entries.is_empty() {
            return Ok(None);
        }

        let mut rng = rand::rng();
        let idx = rng.random_range(0..entries.len());
        let path = &entries[idx];
        let json = fs::read_to_string(path)
            .map_err(|e| format!("failed to read {}: {e}", path.display()))?;
        let genome: Genome = serde_json::from_str(&json)
            .map_err(|e| format!("failed to deserialize favorite: {e}"))?;
        Ok(Some(genome))
    }

    #[test]
    fn lineage_save_load_round_trip() {
        let mut rng = test_rng();
        let child = Genome::random(&mut rng);
        let parent = Genome::random(&mut rng);
        let grandparent = Genome::random(&mut rng);

        let lineage = Lineage {
            child: child.clone(),
            parent: Some(parent.clone()),
            grandparent: Some(grandparent.clone()),
            great_grandparent: None,
        };

        let dir = test_data_dir();
        save_lineage_to(&dir, &lineage).expect("save should succeed");
        let loaded = load_lineage_from(&dir).expect("load should succeed");

        assert_eq!(loaded.child, child);
        assert_eq!(loaded.parent.as_ref(), Some(&parent));
        assert_eq!(loaded.grandparent.as_ref(), Some(&grandparent));
        assert!(loaded.great_grandparent.is_none());

        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn favorite_save_and_load() {
        let mut rng = test_rng();
        let genome = Genome::random(&mut rng);

        let dir = test_data_dir();
        save_favorite_to(&dir, &genome, "test_fav").expect("save should succeed");

        let loaded = load_random_favorite_from(&dir).expect("load should succeed");
        assert_eq!(loaded, Some(genome));

        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_lineage_missing_file() {
        let dir = test_data_dir();
        // Don't write any file — loading should fail
        let result = load_lineage_from(&dir);
        assert!(result.is_err());

        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_random_favorite_empty_dir() {
        let dir = test_data_dir();
        // Empty dir — should return Ok(None)
        let result = load_random_favorite_from(&dir).expect("should not error on empty dir");
        assert!(result.is_none());

        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }
}
