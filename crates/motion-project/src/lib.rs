//! Versioned project documents and explicit, bounded local asset management.
mod assets;
pub use assets::{import_asset, relink_asset, remove_asset, MAX_IMPORT_BYTES};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeSet,
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

pub const FORMAT_VERSION: u32 = 1;
pub const MAX_DOCUMENT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_SOURCE_BYTES: usize = 1024 * 1024;
pub const MAX_ASSETS: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AssetKind {
    Image,
    Audio,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AssetReference {
    pub id: String,
    pub path: String,
    pub kind: AssetKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectDocument {
    pub format_version: u32,
    pub source: String,
    pub assets: Vec<AssetReference>,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Project I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("Project JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Project screenplay: {0}")]
    Script(#[from] motion_script::Error),
    #[error("Project validation: {0}")]
    Validation(String),
    #[error("Unsupported project format version {0}; supported version is 1")]
    UnsupportedVersion(u32),
}

impl ProjectDocument {
    pub fn new(source: impl Into<String>) -> Result<Self, Error> {
        let document = Self {
            format_version: FORMAT_VERSION,
            source: source.into(),
            assets: vec![],
        };
        document.validate()?;
        Ok(document)
    }
    pub fn validate(&self) -> Result<(), Error> {
        if self.format_version != FORMAT_VERSION {
            return Err(Error::UnsupportedVersion(self.format_version));
        }
        if self.source.len() > MAX_SOURCE_BYTES {
            return invalid("screenplay exceeds one MiB");
        }
        if self.assets.len() > MAX_ASSETS {
            return invalid("more than 4096 asset references");
        }
        motion_script::parse(&self.source)?;
        let mut ids = BTreeSet::new();
        for asset in &self.assets {
            if asset.id.trim().is_empty() || asset.id.len() > 256 || !ids.insert(&asset.id) {
                return invalid("asset identifiers must be nonempty, unique and at most 256 bytes");
            }
            validate_path(&asset.path)?;
        }
        Ok(())
    }
}
fn invalid<T>(message: &str) -> Result<T, Error> {
    Err(Error::Validation(message.into()))
}
fn validate_path(path: &str) -> Result<(), Error> {
    if path.is_empty()
        || path.len() > 4096
        || path
            .chars()
            .any(|c| c.is_control() || c == '\\' || c == ':')
        || path
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
    {
        return invalid(
            "asset path must use portable relative slash-separated components without traversal",
        );
    }
    Ok(())
}

pub fn encode(document: &ProjectDocument) -> Result<Vec<u8>, Error> {
    document.validate()?;
    let bytes = serde_json::to_vec_pretty(document)?;
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return invalid("project exceeds four MiB");
    }
    Ok(bytes)
}
pub fn decode(bytes: &[u8]) -> Result<ProjectDocument, Error> {
    if bytes.len() > MAX_DOCUMENT_BYTES {
        return invalid("project exceeds four MiB");
    }
    // Read the version before the current schema so future documents receive an actionable diagnostic.
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    if let Some(version) = value.get("format_version").and_then(|v| v.as_u64()) {
        if version != u64::from(FORMAT_VERSION) {
            return Err(match u32::try_from(version) {
                Ok(v) => Error::UnsupportedVersion(v),
                Err(_) => Error::Validation(format!(
                    "unsupported project format version {version}; supported version is 1"
                )),
            });
        }
    }
    let document: ProjectDocument = serde_json::from_slice(bytes)?;
    document.validate()?;
    Ok(document)
}
pub fn load(path: &Path) -> Result<ProjectDocument, Error> {
    let file = fs::File::open(path)?;
    let mut bytes = vec![];
    file.take((MAX_DOCUMENT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    decode(&bytes)
}

static TEMP_ID: AtomicU64 = AtomicU64::new(0);
/// Save via an exclusive temporary file in the destination directory and rename.
/// Failed validation or writes leave the previous destination untouched. No fallback
/// removes the old file on platforms that cannot atomically replace it.
pub fn save_atomic(path: &Path, document: &ProjectDocument) -> Result<(), Error> {
    let bytes = encode(document)?;
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut created = None;
    for _ in 0..32 {
        let temp = parent.join(format!(
            ".kilsat-{}-{}.tmp",
            std::process::id(),
            TEMP_ID.fetch_add(1, Ordering::Relaxed)
        ));
        match OpenOptions::new().write(true).create_new(true).open(&temp) {
            Ok(file) => {
                created = Some((temp, file));
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.into()),
        }
    }
    let (temp, mut file) =
        created.ok_or_else(|| Error::Validation("cannot reserve project temporary file".into()))?;
    let result = (|| -> Result<(), Error> {
        file.write_all(&bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temp, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temp);
    }
    result
}

/// Resolve an existing regular asset file beneath the canonical project directory.
/// Canonicalization rejects symlinks escaping that directory. As with any returned
/// filesystem path, callers must consider concurrent filesystem changes before use.
pub fn resolve_asset(project_path: &Path, asset: &AssetReference) -> Result<PathBuf, Error> {
    validate_path(&asset.path)?;
    let parent = project_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let root = parent.canonicalize()?;
    let resolved = root.join(&asset.path).canonicalize()?;
    if !resolved.starts_with(&root) || !resolved.is_file() {
        return invalid("asset must be a regular file inside the project directory");
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;
    pub(super) const SOURCE: &str =
        "# preserve this comment\ncharacters: {}\nscenes: [{id: intro, duration: 2, events: []}]\n";
    pub(super) struct Dir(pub(super) PathBuf);
    impl Dir {
        pub(super) fn new() -> Self {
            let p = std::env::temp_dir().join(format!(
                "kilsat-project-test-{}-{}",
                std::process::id(),
                TEMP_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Dir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    #[test]
    fn roundtrip_preserves_exact_source_and_assets() {
        let mut doc = ProjectDocument::new(SOURCE).unwrap();
        doc.assets.push(AssetReference {
            id: "background".into(),
            path: "assets/room.png".into(),
            kind: AssetKind::Image,
        });
        let decoded = decode(&encode(&doc).unwrap()).unwrap();
        assert_eq!(decoded.source, SOURCE);
        assert_eq!(decoded.assets, doc.assets);
        assert!(decode(br#"{"format_version":2,"future_field":true}"#)
            .unwrap_err()
            .to_string()
            .contains("version 2"));
        assert!(decode(&vec![b' '; MAX_DOCUMENT_BYTES + 1]).is_err());
        assert!(ProjectDocument::new("invalid yaml: [").is_err());
    }
    #[test]
    fn rejects_nonportable_paths_duplicate_ids_and_budgets() {
        let mut doc = ProjectDocument::new(SOURCE).unwrap();
        for path in [
            "/absolute",
            "../escape",
            "a/../b",
            "a//b",
            "a\\b",
            "C:drive",
            "./a",
            "a/",
            "a\0b",
        ] {
            doc.assets = vec![AssetReference {
                id: "x".into(),
                path: path.into(),
                kind: AssetKind::Other,
            }];
            assert!(doc.validate().is_err(), "{path}");
        }
        let asset = AssetReference {
            id: "x".into(),
            path: "safe".into(),
            kind: AssetKind::Other,
        };
        doc.assets = vec![asset.clone(), asset.clone()];
        assert!(doc.validate().is_err());
        doc.assets = vec![asset; MAX_ASSETS + 1];
        assert!(doc.validate().is_err());
        doc.assets.clear();
        doc.source = " ".repeat(MAX_SOURCE_BYTES + 1);
        assert!(doc.validate().is_err());
    }
    #[test]
    fn atomic_save_validates_before_replacement() {
        let dir = Dir::new();
        let path = dir.0.join("project.kilsat");
        let mut doc = ProjectDocument::new(SOURCE).unwrap();
        save_atomic(&path, &doc).unwrap();
        doc.source = "broken".into();
        assert!(save_atomic(&path, &doc).is_err());
        assert_eq!(load(&path).unwrap().source, SOURCE);
        doc.source = SOURCE.replace("intro", "outro");
        save_atomic(&path, &doc).unwrap();
        assert_eq!(load(&path).unwrap().source, doc.source);
        assert_eq!(fs::read_dir(&dir.0).unwrap().count(), 1);
    }
    #[test]
    fn resolves_files_and_rejects_directories_and_symlink_escape() {
        let dir = Dir::new();
        let outside = Dir::new();
        let project = dir.0.join("a.kilsat");
        fs::write(dir.0.join("safe.wav"), b"audio").unwrap();
        let mut asset = AssetReference {
            id: "x".into(),
            path: "safe.wav".into(),
            kind: AssetKind::Audio,
        };
        assert_eq!(
            resolve_asset(&project, &asset).unwrap(),
            dir.0.join("safe.wav").canonicalize().unwrap()
        );
        fs::create_dir(dir.0.join("folder")).unwrap();
        asset.path = "folder".into();
        assert!(resolve_asset(&project, &asset).is_err());
        #[cfg(unix)]
        {
            fs::write(outside.0.join("secret"), b"secret").unwrap();
            std::os::unix::fs::symlink(&outside.0, dir.0.join("escape")).unwrap();
            asset.path = "escape/secret".into();
            assert!(resolve_asset(&project, &asset).is_err());
        }
    }
}
