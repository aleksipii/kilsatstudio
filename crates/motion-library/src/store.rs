//! Content-addressed store (CAS) for large asset blobs kept outside Git.
//!
//! Layout: `<root>/sha256/<first two hex>/<64 hex>`. Writes go to
//! `<root>/tmp/` and are renamed atomically into place. Reads re-hash the file;
//! a mismatching file is deleted and reported as [`StoreError::Corrupt`].
//! The root is always an explicit parameter; [`ContentStore::default_root`]
//! only suggests `KILSAT_ASSET_STORE` or `~/Library/Caches/KilsatStudio/assets`.
use crate::manifest::hex;
use sha2::{Digest as _, Sha256};
use std::fmt;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

/// Default per-object limit (1 GiB), matching `geometry.bytes` of blob manifests.
pub const DEFAULT_MAX_OBJECT_BYTES: u64 = crate::manifest::MAX_BLOB_BYTES;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("store I/O at {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid sha256 digest `{0}` (expected 64 lowercase hex digits)")]
    InvalidDigest(String),
    #[error("object {0} is not in the store")]
    Missing(Digest),
    #[error("object {expected} was corrupt (content hashes to {actual}); the file was removed")]
    Corrupt { expected: Digest, actual: String },
    #[error("object of {size} bytes exceeds the store limit of {limit} bytes")]
    TooLarge { size: u64, limit: u64 },
}

/// Lowercase hex SHA-256 digest.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Digest(String);
impl Digest {
    pub fn parse(s: &str) -> Result<Digest, StoreError> {
        if s.len() == 64
            && s.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            Ok(Digest(s.to_owned()))
        } else {
            Err(StoreError::InvalidDigest(s.to_owned()))
        }
    }
    pub fn of(bytes: &[u8]) -> Digest {
        Digest(hex(&Sha256::digest(bytes)))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone)]
pub struct ContentStore {
    root: PathBuf,
    max_object_bytes: u64,
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> StoreError + '_ {
    move |source| StoreError::Io {
        path: path.to_owned(),
        source,
    }
}

impl ContentStore {
    /// Open (creating if needed) a store rooted at `root`.
    pub fn open(root: impl Into<PathBuf>) -> Result<ContentStore, StoreError> {
        let root = root.into();
        for dir in [root.join("sha256"), root.join("tmp")] {
            fs::create_dir_all(&dir).map_err(io(&dir))?;
        }
        Ok(ContentStore {
            root,
            max_object_bytes: DEFAULT_MAX_OBJECT_BYTES,
        })
    }
    pub fn with_max_object_bytes(mut self, limit: u64) -> ContentStore {
        self.max_object_bytes = limit;
        self
    }
    /// Suggested root: `$KILSAT_ASSET_STORE`, else
    /// `$HOME/Library/Caches/KilsatStudio/assets`. Never a repository path.
    pub fn default_root() -> Option<PathBuf> {
        if let Some(p) = std::env::var_os("KILSAT_ASSET_STORE").filter(|p| !p.is_empty()) {
            return Some(PathBuf::from(p));
        }
        std::env::var_os("HOME")
            .filter(|h| !h.is_empty())
            .map(|h| PathBuf::from(h).join("Library/Caches/KilsatStudio/assets"))
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn path_for(&self, digest: &Digest) -> PathBuf {
        self.root
            .join("sha256")
            .join(&digest.0[..2])
            .join(&digest.0)
    }
    pub fn contains(&self, digest: &Digest) -> bool {
        self.path_for(digest).is_file()
    }

    /// Store `bytes` and return their digest. Idempotent; atomic per object.
    pub fn put(&self, bytes: &[u8]) -> Result<Digest, StoreError> {
        let size = bytes.len() as u64;
        if size > self.max_object_bytes {
            return Err(StoreError::TooLarge {
                size,
                limit: self.max_object_bytes,
            });
        }
        let digest = Digest::of(bytes);
        let target = self.path_for(&digest);
        let dir = target.parent().unwrap_or(&self.root).to_owned();
        fs::create_dir_all(&dir).map_err(io(&dir))?;
        let tmp = self.root.join("tmp").join(format!(
            "{}-{}-{}.tmp",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed),
            &digest.0[..16]
        ));
        let write = || -> std::io::Result<()> {
            let mut file = fs::File::create(&tmp)?;
            file.write_all(bytes)?;
            file.sync_all()
        };
        if let Err(e) = write() {
            let _ = fs::remove_file(&tmp);
            return Err(io(&tmp)(e));
        }
        if let Err(e) = fs::rename(&tmp, &target) {
            let _ = fs::remove_file(&tmp);
            return Err(io(&target)(e));
        }
        Ok(digest)
    }

    /// Read and verify an object. A corrupt object is removed.
    pub fn get(&self, digest: &Digest) -> Result<Vec<u8>, StoreError> {
        let path = self.path_for(digest);
        let meta = match fs::metadata(&path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(StoreError::Missing(digest.clone()))
            }
            Err(e) => return Err(io(&path)(e)),
        };
        if meta.len() > self.max_object_bytes {
            return Err(StoreError::TooLarge {
                size: meta.len(),
                limit: self.max_object_bytes,
            });
        }
        let mut bytes = Vec::with_capacity(meta.len() as usize);
        fs::File::open(&path)
            .and_then(|f| f.take(self.max_object_bytes + 1).read_to_end(&mut bytes))
            .map_err(io(&path))?;
        let actual = Digest::of(&bytes);
        if actual != *digest {
            fs::remove_file(&path).map_err(io(&path))?;
            return Err(StoreError::Corrupt {
                expected: digest.clone(),
                actual: actual.0,
            });
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root(tag: &str) -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "motion-library-cas-{tag}-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&p);
        p
    }

    #[test]
    fn put_get_round_trip_and_layout() {
        let root = temp_root("rt");
        let store = ContentStore::open(&root).unwrap();
        let d = store.put(b"hello").unwrap();
        assert_eq!(
            d.as_str(),
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
        assert!(store.path_for(&d).ends_with(
            "sha256/2c/2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        ));
        assert_eq!(store.get(&d).unwrap(), b"hello");
        // Idempotent put, no temp files left behind.
        assert_eq!(store.put(b"hello").unwrap(), d);
        assert_eq!(fs::read_dir(root.join("tmp")).unwrap().count(), 0);
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn corrupt_object_is_removed_and_reported() {
        let root = temp_root("corrupt");
        let store = ContentStore::open(&root).unwrap();
        let d = store.put(b"original bytes").unwrap();
        fs::write(store.path_for(&d), b"tampered").unwrap();
        assert!(matches!(store.get(&d), Err(StoreError::Corrupt { .. })));
        assert!(!store.contains(&d));
        assert!(matches!(store.get(&d), Err(StoreError::Missing(_))));
        // Re-putting heals the entry.
        store.put(b"original bytes").unwrap();
        assert_eq!(store.get(&d).unwrap(), b"original bytes");
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn limits_and_digest_parsing() {
        let root = temp_root("limit");
        let store = ContentStore::open(&root).unwrap().with_max_object_bytes(4);
        assert!(matches!(
            store.put(b"12345"),
            Err(StoreError::TooLarge { size: 5, limit: 4 })
        ));
        let d = store.put(b"1234").unwrap();
        // An oversized file on disk is refused before reading.
        fs::write(store.path_for(&d), b"123456").unwrap();
        assert!(matches!(store.get(&d), Err(StoreError::TooLarge { .. })));
        assert!(Digest::parse("ABC").is_err());
        assert!(Digest::parse(&"A".repeat(64)).is_err());
        assert!(Digest::parse(&"a".repeat(64)).is_ok());
        assert!(matches!(
            store.get(&Digest::parse(&"0".repeat(64)).unwrap()),
            Err(StoreError::Missing(_))
        ));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn default_root_is_never_relative() {
        if let Some(p) = ContentStore::default_root() {
            assert!(p.is_absolute() || std::env::var_os("KILSAT_ASSET_STORE").is_some());
        }
    }
}
