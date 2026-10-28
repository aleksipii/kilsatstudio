//! Loading a pack's `library/` tree, cross-reference checks, licence gates and search.
use crate::manifest::{Kind, Manifest, ManifestError, SELF_INSTANCE};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;
pub const MAX_MANIFESTS: usize = 10_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Policy {
    /// Non-distributable assets produce warnings.
    Authoring,
    /// Non-distributable assets are errors.
    Distribution,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// Manifest path relative to the pack root.
    pub file: PathBuf,
    pub id: String,
    pub message: String,
}

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{file}: {source}")]
    Manifest {
        file: PathBuf,
        source: ManifestError,
    },
    #[error("{0}: symbolic links are not allowed in the library tree")]
    Symlink(PathBuf),
    #[error("{0}: unexpected entry; manifests live in library/{{backgrounds,props,locations,sets}}/<id>.json")]
    Layout(PathBuf),
    #[error("{file}: file name must be `{id}.json`")]
    FileName { file: PathBuf, id: String },
    #[error("{file}: kind `{kind}` belongs in library/{}/", kind.directory())]
    Directory { file: PathBuf, kind: Kind },
    #[error("{file}: duplicate id `{id}` (also in {other})")]
    Duplicate {
        file: PathBuf,
        id: String,
        other: PathBuf,
    },
    #[error("{file}: {size} bytes exceeds the manifest limit of {limit}")]
    TooLarge {
        file: PathBuf,
        size: u64,
        limit: u64,
    },
    #[error("library exceeds {MAX_MANIFESTS} manifests")]
    TooMany,
    #[error("{file}: {field}: {message}")]
    Reference {
        file: PathBuf,
        field: String,
        message: String,
    },
    #[error("{file}: `{id}` is not distributable: {reason}")]
    NotDistributable {
        file: PathBuf,
        id: String,
        reason: String,
    },
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub manifest: Manifest,
    /// Path relative to the pack root.
    pub file: PathBuf,
}

/// Search filter; every set criterion must match.
#[derive(Debug, Clone, Default)]
pub struct Query {
    pub kind: Option<Kind>,
    /// All of these tags must be present.
    pub tags_all: Vec<String>,
    /// Case-insensitive substring of the id, name or a tag.
    pub text: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Library {
    root: PathBuf,
    entries: BTreeMap<String, Entry>,
}

fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> LibraryError + '_ {
    move |source| LibraryError::Io {
        path: path.to_owned(),
        source,
    }
}

fn sorted_dir(path: &Path) -> Result<Vec<PathBuf>, LibraryError> {
    let mut out = Vec::new();
    for entry in fs::read_dir(path).map_err(io_err(path))? {
        out.push(entry.map_err(io_err(path))?.path());
    }
    out.sort();
    Ok(out)
}

impl Library {
    /// Load `pack_root/library/**/*.json`, validate every manifest and all
    /// references, and apply the licence policy.
    pub fn load(pack_root: &Path, policy: Policy) -> Result<(Library, Vec<Warning>), LibraryError> {
        let lib_dir = pack_root.join("library");
        let meta = fs::symlink_metadata(&lib_dir).map_err(io_err(&lib_dir))?;
        if meta.file_type().is_symlink() {
            return Err(LibraryError::Symlink(lib_dir));
        }
        let mut entries: BTreeMap<String, Entry> = BTreeMap::new();
        for dir in sorted_dir(&lib_dir)? {
            let rel_dir = dir.strip_prefix(pack_root).unwrap_or(&dir).to_owned();
            let meta = fs::symlink_metadata(&dir).map_err(io_err(&dir))?;
            if meta.file_type().is_symlink() {
                return Err(LibraryError::Symlink(rel_dir));
            }
            if meta.is_file() {
                if is_json(&dir) {
                    return Err(LibraryError::Layout(rel_dir));
                }
                continue; // README and similar files are ignored.
            }
            let dir_name = dir.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let dir_kind = Kind::from_directory(dir_name).ok_or(LibraryError::Layout(rel_dir))?;
            for file in sorted_dir(&dir)? {
                let rel = file.strip_prefix(pack_root).unwrap_or(&file).to_owned();
                let meta = fs::symlink_metadata(&file).map_err(io_err(&file))?;
                if meta.file_type().is_symlink() {
                    return Err(LibraryError::Symlink(rel));
                }
                if meta.is_dir() {
                    return Err(LibraryError::Layout(rel));
                }
                if !is_json(&file) {
                    continue;
                }
                if meta.len() > MAX_MANIFEST_BYTES {
                    return Err(LibraryError::TooLarge {
                        file: rel,
                        size: meta.len(),
                        limit: MAX_MANIFEST_BYTES,
                    });
                }
                if entries.len() >= MAX_MANIFESTS {
                    return Err(LibraryError::TooMany);
                }
                let bytes = fs::read(&file).map_err(io_err(&file))?;
                let wrap = |source| LibraryError::Manifest {
                    file: rel.clone(),
                    source,
                };
                let manifest = Manifest::from_json(&bytes).map_err(wrap)?;
                manifest.validate(pack_root).map_err(wrap)?;
                let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or("");
                if stem != manifest.id {
                    return Err(LibraryError::FileName {
                        file: rel,
                        id: manifest.id,
                    });
                }
                if manifest.kind != dir_kind {
                    return Err(LibraryError::Directory {
                        file: rel,
                        kind: manifest.kind,
                    });
                }
                if let Some(prev) = entries.get(&manifest.id) {
                    return Err(LibraryError::Duplicate {
                        file: rel,
                        id: manifest.id,
                        other: prev.file.clone(),
                    });
                }
                entries.insert(
                    manifest.id.clone(),
                    Entry {
                        manifest,
                        file: rel,
                    },
                );
            }
        }
        let library = Library {
            root: pack_root.to_owned(),
            entries,
        };
        library.check_references()?;
        let mut warnings = Vec::new();
        for entry in library.entries.values() {
            let reasons = library.distribution_blockers(&entry.manifest.id);
            if reasons.is_empty() {
                continue;
            }
            let reason = reasons.join("; ");
            match policy {
                Policy::Distribution => {
                    return Err(LibraryError::NotDistributable {
                        file: entry.file.clone(),
                        id: entry.manifest.id.clone(),
                        reason,
                    })
                }
                Policy::Authoring => warnings.push(Warning {
                    file: entry.file.clone(),
                    id: entry.manifest.id.clone(),
                    message: format!("not distributable: {reason}"),
                }),
            }
        }
        Ok((library, warnings))
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn get(&self, id: &str) -> Option<&Manifest> {
        self.entries.get(id).map(|e| &e.manifest)
    }
    pub fn entry(&self, id: &str) -> Option<&Entry> {
        self.entries.get(id)
    }
    /// All manifests ordered by id.
    pub fn iter(&self) -> impl Iterator<Item = &Manifest> {
        self.entries.values().map(|e| &e.manifest)
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn count(&self, kind: Kind) -> usize {
        self.iter().filter(|m| m.kind == kind).count()
    }

    /// Deterministic search ordered by id.
    pub fn search(&self, query: &Query) -> Vec<&Manifest> {
        let text = query.text.as_ref().map(|t| t.to_lowercase());
        self.iter()
            .filter(|m| query.kind.is_none_or(|k| m.kind == k))
            .filter(|m| query.tags_all.iter().all(|t| m.tags.contains(t)))
            .filter(|m| {
                text.as_ref().is_none_or(|t| {
                    m.id.contains(t.as_str())
                        || m.name.to_lowercase().contains(t.as_str())
                        || m.tags.iter().any(|tag| tag.contains(t.as_str()))
                })
            })
            .collect()
    }

    /// Reasons `id` (including everything it references, transitively) may not
    /// be distributed. Empty means distributable.
    pub fn distribution_blockers(&self, id: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = BTreeSet::new();
        self.collect_blockers(id, &mut seen, &mut out);
        out
    }
    fn collect_blockers(&self, id: &str, seen: &mut BTreeSet<String>, out: &mut Vec<String>) {
        if !seen.insert(id.to_owned()) {
            return;
        }
        let Some(m) = self.get(id) else {
            out.push(format!("{id}: missing"));
            return;
        };
        for b in m.license.blockers() {
            out.push(format!("{id}: {b}"));
        }
        for part in &m.parts {
            self.collect_blockers(&part.asset, seen, out);
        }
        if let Some(loc) = &m.location {
            self.collect_blockers(loc, seen, out);
        }
    }

    /// Anchor ids an instance exposes inside a location or set context.
    fn instance_anchor(&self, owner: &Manifest, instance: &str, anchor: &str) -> Option<bool> {
        if instance == SELF_INSTANCE && owner.kind == Kind::Location {
            let in_room = owner
                .room_art()
                .is_some_and(|art| art.anchors.iter().any(|a| a.id() == anchor));
            return Some(in_room || owner.anchor(anchor).is_some());
        }
        let part = owner.parts.iter().find(|p| p.instance == instance)?;
        Some(self.get(&part.asset)?.anchor(anchor).is_some())
    }

    fn check_references(&self) -> Result<(), LibraryError> {
        for entry in self.entries.values() {
            let m = &entry.manifest;
            let fail = |field: String, message: String| LibraryError::Reference {
                file: entry.file.clone(),
                field,
                message,
            };
            for (i, part) in m.parts.iter().enumerate() {
                let target = self.get(&part.asset).ok_or_else(|| {
                    fail(
                        format!("parts[{i}].asset"),
                        format!("`{}` does not exist", part.asset),
                    )
                })?;
                if !matches!(target.kind, Kind::Prop | Kind::Background) {
                    return Err(fail(
                        format!("parts[{i}].asset"),
                        format!(
                            "`{}` is a {}, expected prop or background",
                            part.asset, target.kind
                        ),
                    ));
                }
                if let Some(v) = &part.variant {
                    if target.variant(v).is_none() {
                        return Err(fail(
                            format!("parts[{i}].variant"),
                            format!("`{}` has no variant `{v}`", part.asset),
                        ));
                    }
                }
            }
            let Some(loc_id) = &m.location else { continue };
            let loc = self
                .get(loc_id)
                .ok_or_else(|| fail("location".into(), format!("`{loc_id}` does not exist")))?;
            if loc.kind != Kind::Location {
                return Err(fail(
                    "location".into(),
                    format!("`{loc_id}` is a {}, expected location", loc.kind),
                ));
            }
            for (i, part) in m.parts.iter().enumerate() {
                if loc.parts.iter().any(|p| p.instance == part.instance) {
                    return Err(fail(
                        format!("parts[{i}].instance"),
                        format!("`{}` is already an instance of `{loc_id}`", part.instance),
                    ));
                }
            }
            for (i, mark) in m.marks.iter().enumerate() {
                let Some((instance, anchor)) = mark.anchor_ref() else {
                    continue;
                };
                let found = if m.parts.iter().any(|p| p.instance == instance) {
                    self.instance_anchor(m, instance, anchor)
                } else {
                    self.instance_anchor(loc, instance, anchor)
                };
                match found {
                    Some(true) => {}
                    Some(false) => {
                        return Err(fail(
                            format!("marks[{i}].anchor"),
                            format!("instance `{instance}` has no anchor `{anchor}`"),
                        ))
                    }
                    None => {
                        return Err(fail(
                            format!("marks[{i}].anchor"),
                            format!("no instance `{instance}` in the set or `{loc_id}`"),
                        ))
                    }
                }
            }
        }
        Ok(())
    }
}

fn is_json(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "json")
}
