use crate::{Reference, Result, Signature};
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Serialize, Deserialize)]
pub struct CacheEntry {
    pub mtime: SystemTime,
    pub signatures: Vec<Signature>,
    pub references: Vec<Reference>,
}

pub struct Cache {
    dir: PathBuf,
}

impl Cache {
    pub fn new(dir: PathBuf) -> Self {
        Self { dir }
    }

    fn cache_path(&self, file: &Path) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        file.hash(&mut hasher);
        self.dir.join(format!("{:x}.bin", hasher.finish()))
    }

    pub fn get(&self, file: &Path) -> Result<Option<CacheEntry>> {
        let cache_path = self.cache_path(file);
        if !cache_path.exists() {
            return Ok(None);
        }
        let data = std::fs::read(&cache_path)?;
        let entry: CacheEntry = match bincode::deserialize(&data) {
            Ok(e) => e,
            Err(_) => return Ok(None),
        };
        let current_mtime = std::fs::metadata(file)?.modified()?;
        if entry.mtime != current_mtime {
            return Ok(None);
        }
        Ok(Some(entry))
    }

    pub fn put(&self, file: &Path, entry: &CacheEntry) -> Result<()> {
        std::fs::create_dir_all(&self.dir)?;
        let cache_path = self.cache_path(file);
        let data = bincode::serialize(entry).map_err(|e| crate::Error::Other(e.to_string()))?;
        std::fs::write(cache_path, data)?;
        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        if self.dir.exists() {
            std::fs::remove_dir_all(&self.dir)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Signature, SignatureKind, Visibility};

    #[test]
    fn cache_hit_on_unchanged_file() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().join(".sigdiff/cache"));
        let file = dir.path().join("t.rs");
        std::fs::write(&file, "fn hello() {}").unwrap();
        let entry = CacheEntry {
            mtime: std::fs::metadata(&file).unwrap().modified().unwrap(),
            signatures: vec![Signature {
                file: "t.rs".into(),
                name: "hello".into(),
                kind: SignatureKind::Function,
                visibility: Visibility::Public,
                text: "fn hello()".into(),
                line: 1,
                parent: None,
            }],
            references: vec![],
        };
        cache.put(&file, &entry).unwrap();
        let cached = cache.get(&file).unwrap();
        assert!(cached.is_some());
        assert_eq!(cached.unwrap().signatures.len(), 1);
    }

    #[test]
    fn cache_miss_on_changed_mtime() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().join(".sigdiff/cache"));
        let file = dir.path().join("t.rs");
        std::fs::write(&file, "fn hello() {}").unwrap();
        let entry = CacheEntry {
            mtime: std::time::SystemTime::UNIX_EPOCH,
            signatures: vec![],
            references: vec![],
        };
        cache.put(&file, &entry).unwrap();
        assert!(cache.get(&file).unwrap().is_none());
    }

    #[test]
    fn cache_clear_removes_directory() {
        let dir = tempfile::tempdir().unwrap();
        let cache_dir = dir.path().join(".sigdiff/cache");
        let cache = Cache::new(cache_dir.clone());
        let file = dir.path().join("t.rs");
        std::fs::write(&file, "fn hello() {}").unwrap();
        let entry = CacheEntry {
            mtime: std::fs::metadata(&file).unwrap().modified().unwrap(),
            signatures: vec![],
            references: vec![],
        };
        cache.put(&file, &entry).unwrap();
        assert!(cache_dir.exists());
        cache.clear().unwrap();
        assert!(!cache_dir.exists());
    }

    #[test]
    fn cache_clear_on_nonexistent_dir_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().join("nonexistent/cache"));
        assert!(cache.clear().is_ok());
    }

    #[test]
    fn cache_miss_on_nonexistent_cache_file() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().join(".sigdiff/cache"));
        let file = dir.path().join("t.rs");
        // File doesn't exist in cache at all
        std::fs::write(&file, "fn hello() {}").unwrap();
        assert!(cache.get(&file).unwrap().is_none());
    }

    #[test]
    fn cache_miss_on_corrupted_data() {
        let dir = tempfile::tempdir().unwrap();
        let cache_dir = dir.path().join(".sigdiff/cache");
        let cache = Cache::new(cache_dir.clone());
        let file = dir.path().join("t.rs");
        std::fs::write(&file, "fn hello() {}").unwrap();

        // Write corrupted data to the cache path
        std::fs::create_dir_all(&cache_dir).unwrap();
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        std::hash::Hash::hash(&file, &mut hasher);
        let cache_path = cache_dir.join(format!("{:x}.bin", std::hash::Hasher::finish(&hasher)));
        std::fs::write(cache_path, b"corrupted data").unwrap();

        // Should return None (graceful handling), not error
        assert!(cache.get(&file).unwrap().is_none());
    }

    #[test]
    fn cache_stores_and_retrieves_references() {
        let dir = tempfile::tempdir().unwrap();
        let cache = Cache::new(dir.path().join(".sigdiff/cache"));
        let file = dir.path().join("t.rs");
        std::fs::write(&file, "fn hello() {}").unwrap();
        let entry = CacheEntry {
            mtime: std::fs::metadata(&file).unwrap().modified().unwrap(),
            signatures: vec![],
            references: vec![crate::Reference {
                file: "t.rs".into(),
                name: "world".into(),
                line: 5,
            }],
        };
        cache.put(&file, &entry).unwrap();
        let cached = cache.get(&file).unwrap().unwrap();
        assert_eq!(cached.references.len(), 1);
        assert_eq!(cached.references[0].name, "world");
    }
}
