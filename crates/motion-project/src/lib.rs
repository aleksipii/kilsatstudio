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

pub const FORMAT_VERSION: u32 = 2;
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
    pub art: motion_art::Document,
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
    #[error("Unsupported project format version {0}; supported version is 2")]
    UnsupportedVersion(u32),
}

impl ProjectDocument {
    pub fn new(source: impl Into<String>) -> Result<Self, Error> {
        let document = Self {
            format_version: FORMAT_VERSION,
            source: source.into(),
            assets: vec![],
            art: motion_art::Document::default(),
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
        let script = motion_script::parse(&self.source)?;
        self.art
            .validate()
            .map_err(|e| Error::Validation(e.to_string()))?;
        let mut ids = BTreeSet::new();
        for asset in &self.assets {
            if asset.id.trim().is_empty() || asset.id.len() > 256 || !ids.insert(&asset.id) {
                return invalid("asset identifiers must be nonempty, unique and at most 256 bytes");
            }
            validate_path(&asset.path)?;
        }
        for group in &self.art.groups {
            if !script.scenes.iter().any(|scene| scene.id == group.scene_id) {
                return invalid("artwork group references a scene that does not exist");
            }
        }
        for layer in &self.art.layers {
            let Some(scene) = script
                .scenes
                .iter()
                .find(|scene| scene.id == layer.scene_id)
            else {
                return invalid("artwork references a scene that does not exist");
            };
            if layer.active.is_some_and(|range| range.end > scene.duration) {
                return invalid("artwork active interval exceeds its scene duration");
            }
            if let Some(shape) = &layer.clip_shape {
                shape
                    .validate_scene_duration(scene.duration)
                    .map_err(|error| Error::Validation(error.to_string()))?;
            }
            if layer
                .attachment
                .as_ref()
                .is_some_and(|actor| !script.characters.contains_key(actor))
            {
                return invalid("artwork attachment references a character that does not exist");
            }
            if let motion_art::Content::Image { asset_id, .. } = &layer.content {
                if !self
                    .assets
                    .iter()
                    .any(|asset| asset.id == *asset_id && asset.kind == AssetKind::Image)
                {
                    return invalid("artwork image must reference an image asset in this project");
                }
            }
        }
        Ok(())
    }
    /// Apply an artwork command only when the complete project remains valid.
    pub fn apply_art(&mut self, command: motion_art::Command) -> Result<(), Error> {
        let mut candidate = self.clone();
        candidate
            .art
            .apply(command)
            .map_err(|e| Error::Validation(e.to_string()))?;
        encode(&candidate)?;
        *self = candidate;
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
    // Version 1 has no artwork field. Migration is explicit and strict so an
    // unknown/artwork field cannot be accidentally discarded by an old schema.
    let value: serde_json::Value = serde_json::from_slice(bytes)?;
    let version = value
        .get("format_version")
        .and_then(|v| v.as_u64())
        .ok_or_else(|| Error::Validation("missing or invalid format_version".into()))?;
    let document: ProjectDocument = if version == 1 {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct V1 {
            format_version: u32,
            source: String,
            assets: Vec<AssetReference>,
        }
        let old: V1 = serde_json::from_slice(bytes)?;
        debug_assert_eq!(old.format_version, 1);
        ProjectDocument {
            format_version: FORMAT_VERSION,
            source: old.source,
            assets: old.assets,
            art: motion_art::Document::default(),
        }
    } else if version == u64::from(FORMAT_VERSION) {
        serde_json::from_slice(bytes)?
    } else {
        return Err(match u32::try_from(version) {
            Ok(v) => Error::UnsupportedVersion(v),
            Err(_) => Error::Validation(format!(
                "unsupported project format version {version}; supported version is 2"
            )),
        });
    };
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
        assert!(decode(br#"{"format_version":3,"future_field":true}"#)
            .unwrap_err()
            .to_string()
            .contains("version 3"));
        assert!(decode(&vec![b' '; MAX_DOCUMENT_BYTES + 1]).is_err());
        assert!(ProjectDocument::new("invalid yaml: [").is_err());
    }
    #[test]
    fn custom_rig_source_survives_project_roundtrip_and_rejects_invalid_version() {
        let source = "characters:\n  PUPPET:\n    asset: custom\n    rig:\n      version: 1\n      id: puppet\n      joints: [{id: root, parent: null, x: 10, y: -20}]\n      parts: [{id: body, joint: root, points: [[0,0],[10,0],[0,10]], color: [20,40,80,255]}]\nscenes: [{id: intro, duration: 2, events: []}]\n";
        let document = ProjectDocument::new(source).unwrap();
        let reopened = decode(&encode(&document).unwrap()).unwrap();
        assert_eq!(reopened.source, source);
        let script = motion_script::parse(&reopened.source).unwrap();
        let parts = script.characters["PUPPET"]
            .rig
            .as_ref()
            .unwrap()
            .evaluate_parts()
            .unwrap();
        assert_eq!(parts[0].points[1].x, 20.0);
        assert_eq!(parts[0].points[1].y, -20.0);
        assert!(ProjectDocument::new(source.replace("version: 1", "version: 2")).is_err());
    }
    #[test]
    fn artwork_group_missing_scene_is_rejected_before_project_save() {
        let mut document = ProjectDocument::new(SOURCE).unwrap();
        document.art.groups.push(motion_art::Group {
            id: "group".into(),
            scene_id: "missing".into(),
            plane: motion_art::Plane::Foreground,
            name: "Group".into(),
            visible: true,
            opacity: 1.0,
            blend: motion_art::BlendMode::Normal,
            children: Vec::new(),
        });
        document
            .art
            .order
            .push(motion_art::ArtUnit::Group("group".into()));
        assert!(document.art.validate().is_ok());
        let error = encode(&document).unwrap_err().to_string();
        assert!(error.contains("group references a scene"));
    }
    fn rectangle() -> motion_art::Layer {
        motion_art::Layer {
            id: "shape-1".into(),
            scene_id: "intro".into(),
            name: "Shape".into(),
            visible: true,
            plane: motion_art::Plane::Foreground,
            active: None,
            opacity: 0.75,
            attachment: None,
            clip_shape: None,
            blend: motion_art::BlendMode::Normal,
            clip: None,
            content: motion_art::Content::Rectangle {
                rect: motion_art::Rect {
                    x: 1.0,
                    y: 2.0,
                    width: 30.0,
                    height: 40.0,
                },
                color: motion_art::Color {
                    r: 10,
                    g: 20,
                    b: 30,
                    a: 128,
                },
            },
        }
    }
    #[test]
    fn timed_art_roundtrips_and_legacy_v2_defaults_preserve_foreground() {
        let mut doc = ProjectDocument::new(SOURCE).unwrap();
        let mut item = rectangle();
        item.plane = motion_art::Plane::Background;
        item.active = Some(motion_art::TimeRange {
            start: 0.5,
            end: 2.0,
        });
        doc.apply_art(motion_art::Command::Add(item)).unwrap();
        let bytes = encode(&doc).unwrap();
        assert_eq!(decode(&bytes).unwrap().art, doc.art);
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["art"]["layers"][0]["plane"], "background");
        let layer = value["art"]["layers"][0].as_object_mut().unwrap();
        layer.remove("plane");
        layer.remove("active");
        let legacy = decode(&serde_json::to_vec(&value).unwrap()).unwrap();
        assert_eq!(legacy.art.layers[0].plane, motion_art::Plane::Foreground);
        assert_eq!(legacy.art.layers[0].active, None);
        value["art"]["layers"][0]["active"] = serde_json::json!({"start":0,"end":1,"extra":true});
        assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
        value["art"]["layers"][0]["active"] = serde_json::Value::Null;
        value["art"]["layers"][0]["plane"] = serde_json::json!("unknown");
        assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    #[test]
    fn active_art_checks_scene_duration_references_and_atomic_replacement() {
        let mut doc = ProjectDocument::new(SOURCE).unwrap();
        let mut item = rectangle();
        item.active = Some(motion_art::TimeRange {
            start: 0.0,
            end: 2.0,
        });
        doc.apply_art(motion_art::Command::Add(item.clone()))
            .unwrap();
        let before = encode(&doc).unwrap();
        item.active.as_mut().unwrap().end = 2.01;
        assert!(doc
            .apply_art(motion_art::Command::Replace(item.clone()))
            .is_err());
        assert_eq!(encode(&doc).unwrap(), before);
        item.active.as_mut().unwrap().end = 1.0;
        item.scene_id = "missing".into();
        assert!(doc.apply_art(motion_art::Command::Replace(item)).is_err());
        assert_eq!(encode(&doc).unwrap(), before);
        doc.source = SOURCE.replace("duration: 2", "duration: 1");
        assert!(doc.validate().is_err());
    }
    #[test]
    fn explicit_v1_migration_preserves_source_assets_and_resaves_v2() {
        let bytes = serde_json::to_vec(&serde_json::json!({"format_version":1,"source":SOURCE,"assets":[{"id":"room","path":"resources/room.png","kind":"image"}]})).unwrap();
        let mut doc = decode(&bytes).unwrap();
        assert_eq!(doc.format_version, 2);
        assert_eq!(doc.source, SOURCE);
        assert_eq!(doc.assets[0].id, "room");
        assert!(doc.art.layers.is_empty());
        doc.apply_art(motion_art::Command::Add(rectangle()))
            .unwrap();
        let saved = encode(&doc).unwrap();
        let reloaded = decode(&saved).unwrap();
        assert_eq!(reloaded.art, doc.art);
        assert_eq!(reloaded.source, SOURCE);
        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["art"] = serde_json::json!({"layers":[]});
        assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
        let mut value: serde_json::Value = serde_json::from_slice(&saved).unwrap();
        value.as_object_mut().unwrap().remove("art");
        assert!(decode(&serde_json::to_vec(&value).unwrap()).is_err());
    }
    #[test]
    fn rejects_orphan_art_and_removing_referenced_images_transactionally() {
        let mut doc = ProjectDocument::new(SOURCE).unwrap();
        let mut layer = rectangle();
        layer.scene_id = "missing".into();
        assert!(doc.apply_art(motion_art::Command::Add(layer)).is_err());
        assert!(doc.art.layers.is_empty());
        let mut layer = rectangle();
        layer.attachment = Some("missing".into());
        assert!(doc.apply_art(motion_art::Command::Add(layer)).is_err());
        let mut layer = rectangle();
        layer.content = motion_art::Content::Image {
            asset_id: "room".into(),
            rect: motion_art::Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
        };
        assert!(doc
            .apply_art(motion_art::Command::Add(layer.clone()))
            .is_err());
        doc.assets.push(AssetReference {
            id: "room".into(),
            path: "resources/room.png".into(),
            kind: AssetKind::Image,
        });
        doc.apply_art(motion_art::Command::Add(layer)).unwrap();
        assert!(remove_asset(&mut doc, "room").is_err());
        assert_eq!(doc.assets.len(), 1);
        assert_eq!(doc.art.layers.len(), 1);
        doc.source = SOURCE.replace("intro", "outro");
        assert!(doc.validate().is_err());
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
