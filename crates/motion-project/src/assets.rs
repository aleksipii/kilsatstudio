use super::*;

/// Explicit local imports are capped independently from the small project document.
pub const MAX_IMPORT_BYTES: u64 = 256 * 1024 * 1024;

/// Copy a local regular file into `resources/` and append its inventory reference.
/// The copy is published with an exclusive hard link after all bytes are synced;
/// an existing file is never replaced. No file contents are decoded or downloaded.
pub fn import_asset(
    project_path: &Path,
    document: &mut ProjectDocument,
    source: &Path,
    id: &str,
    kind: AssetKind,
) -> Result<AssetReference, Error> {
    import_bounded(project_path, document, source, id, kind, MAX_IMPORT_BYTES)
}

fn import_bounded(
    project_path: &Path,
    document: &mut ProjectDocument,
    source: &Path,
    id: &str,
    kind: AssetKind,
    limit: u64,
) -> Result<AssetReference, Error> {
    let root = project_root(project_path)?;
    let metadata = fs::symlink_metadata(source)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return invalid("import source must be a regular file, not a symlink or directory");
    }
    if metadata.len() > limit {
        return invalid("asset exceeds the 256 MiB import limit");
    }
    // Generated ASCII names avoid platform-specific filename/traversal rules.
    let token = TEMP_ID.fetch_add(1, Ordering::Relaxed);
    let stem = format!("asset-{}-{token}", std::process::id());
    let extension = source
        .extension()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty() && s.len() <= 16 && s.bytes().all(|b| b.is_ascii_alphanumeric()));
    let filename = match extension {
        Some(extension) => format!("{stem}.{extension}"),
        None => stem,
    };
    let asset = AssetReference {
        id: id.into(),
        path: format!("resources/{filename}"),
        kind,
    };
    let mut candidate = document.clone();
    candidate.assets.push(asset.clone());
    encode(&candidate)?;
    let resources = root.join("resources");
    let created_directory = match fs::create_dir(&resources) {
        Ok(()) => true,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => false,
        Err(error) => return Err(error.into()),
    };
    let temp = resources.join(format!(".import-{}-{token}.tmp", std::process::id()));
    let destination = resources.join(&filename);
    let mut owns_temp = false;
    let result = (|| -> Result<(), Error> {
        let metadata = fs::symlink_metadata(&resources)?;
        if !metadata.is_dir()
            || metadata.file_type().is_symlink()
            || resources.canonicalize()? != resources
        {
            return invalid("project resources directory must not be a symlink");
        }
        // Recheck the opened source, then bound the read even if it grows.
        let input = fs::File::open(source)?;
        if !input.metadata()?.is_file() {
            return invalid("import source is not a regular file");
        }
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;
        owns_temp = true;
        let copied = std::io::copy(&mut input.take(limit + 1), &mut output)?;
        if copied > limit {
            return invalid("asset exceeds the 256 MiB import limit");
        }
        output.sync_all()?;
        drop(output);
        fs::hard_link(&temp, &destination)?;
        if let Err(error) = fs::remove_file(&temp) {
            let _ = fs::remove_file(&destination);
            return Err(error.into());
        }
        Ok(())
    })();
    if let Err(error) = result {
        if owns_temp {
            let _ = fs::remove_file(&temp);
        }
        if created_directory {
            let _ = fs::remove_dir(&resources);
        }
        return Err(error);
    }
    *document = candidate;
    Ok(asset)
}

/// Point an existing reference at a regular file inside the canonical project root.
/// This changes inventory metadata only, never the target file or screenplay.
pub fn relink_asset(
    project_path: &Path,
    document: &mut ProjectDocument,
    id: &str,
    path: &str,
) -> Result<(), Error> {
    let mut candidate = document.clone();
    let Some(asset) = candidate.assets.iter_mut().find(|asset| asset.id == id) else {
        return invalid("asset identifier does not exist");
    };
    asset.path = path.into();
    resolve_asset(project_path, asset)?;
    encode(&candidate)?;
    *document = candidate;
    Ok(())
}

/// Remove inventory metadata only. Imported and original files are always retained.
pub fn remove_asset(document: &mut ProjectDocument, id: &str) -> Result<AssetReference, Error> {
    let mut candidate = document.clone();
    let Some(index) = candidate.assets.iter().position(|asset| asset.id == id) else {
        return invalid("asset identifier does not exist");
    };
    let removed = candidate.assets.remove(index);
    encode(&candidate)?;
    *document = candidate;
    Ok(removed)
}

fn project_root(project_path: &Path) -> Result<PathBuf, Error> {
    Ok(project_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."))
        .canonicalize()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::{Dir, SOURCE};

    #[test]
    fn imports_unique_copies_and_removal_never_deletes_files() {
        let root = Dir::new();
        let external = Dir::new();
        let project = root.0.join("project.kilsat");
        let original = external.0.join("voice.wav");
        fs::write(&original, b"original audio").unwrap();
        let mut doc = ProjectDocument::new(SOURCE).unwrap();
        for kind in [AssetKind::Audio, AssetKind::Image, AssetKind::Other] {
            let id = format!("asset-{}", doc.assets.len());
            let reference = import_asset(&project, &mut doc, &original, &id, kind).unwrap();
            assert_eq!(
                fs::read(resolve_asset(&project, &reference).unwrap()).unwrap(),
                b"original audio"
            );
        }
        let before = encode(&doc).unwrap();
        assert!(import_asset(&project, &mut doc, &original, "asset-0", AssetKind::Audio).is_err());
        assert_eq!(encode(&doc).unwrap(), before);
        assert_eq!(fs::read_dir(root.0.join("resources")).unwrap().count(), 3);
        let removed = remove_asset(&mut doc, "asset-0").unwrap();
        assert!(resolve_asset(&project, &removed).is_ok());
        assert_eq!(fs::read(original).unwrap(), b"original audio");
        save_atomic(&project, &doc).unwrap();
        assert_eq!(load(&project).unwrap().assets, doc.assets);
    }

    #[test]
    fn failure_preserves_inventory_and_cleans_partial_copies() {
        let root = Dir::new();
        let source = root.0.join("large.bin");
        fs::write(&source, b"12345").unwrap();
        let project = root.0.join("project.kilsat");
        let mut doc = ProjectDocument::new(SOURCE).unwrap();
        let before = encode(&doc).unwrap();
        assert!(import_bounded(&project, &mut doc, &source, "large", AssetKind::Other, 4).is_err());
        assert!(!root.0.join("resources").exists());
        assert!(import_asset(&project, &mut doc, &root.0, "dir", AssetKind::Other).is_err());
        assert_eq!(encode(&doc).unwrap(), before);
        assert_eq!(fs::read(source).unwrap(), b"12345");
    }

    #[test]
    fn relink_validates_and_preserves_kind_source_and_original_files() {
        let root = Dir::new();
        let project = root.0.join("project.kilsat");
        fs::write(root.0.join("a.png"), b"a").unwrap();
        fs::write(root.0.join("b.png"), b"b").unwrap();
        let mut doc = ProjectDocument::new(SOURCE).unwrap();
        doc.assets.push(AssetReference {
            id: "image".into(),
            path: "a.png".into(),
            kind: AssetKind::Image,
        });
        for path in [
            "../outside.png",
            "missing.png",
            "a.png/../../escape",
            "/absolute",
        ] {
            let before = encode(&doc).unwrap();
            assert!(relink_asset(&project, &mut doc, "image", path).is_err());
            assert_eq!(encode(&doc).unwrap(), before);
        }
        assert!(remove_asset(&mut doc, "missing").is_err());
        assert!(relink_asset(&project, &mut doc, "missing", "b.png").is_err());
        relink_asset(&project, &mut doc, "image", "b.png").unwrap();
        assert_eq!(doc.assets[0].path, "b.png");
        assert_eq!(doc.assets[0].kind, AssetKind::Image);
        assert_eq!(doc.source, SOURCE);
        assert_eq!(fs::read(root.0.join("a.png")).unwrap(), b"a");
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_cannot_redirect_import_or_relink_outside_project() {
        let root = Dir::new();
        let outside = Dir::new();
        let project = root.0.join("project.kilsat");
        let source = outside.0.join("a");
        fs::write(&source, b"private").unwrap();
        let mut doc = ProjectDocument::new(SOURCE).unwrap();
        std::os::unix::fs::symlink(&source, root.0.join("source-link")).unwrap();
        assert!(import_asset(
            &project,
            &mut doc,
            &root.0.join("source-link"),
            "a",
            AssetKind::Other
        )
        .is_err());
        std::os::unix::fs::symlink(&outside.0, root.0.join("resources")).unwrap();
        assert!(import_asset(&project, &mut doc, &source, "a", AssetKind::Other).is_err());
        assert_eq!(fs::read_dir(&outside.0).unwrap().count(), 1);
        doc.assets.push(AssetReference {
            id: "a".into(),
            path: "safe".into(),
            kind: AssetKind::Other,
        });
        assert!(relink_asset(&project, &mut doc, "a", "resources/a").is_err());
        assert_eq!(doc.assets[0].path, "safe");
    }
}
