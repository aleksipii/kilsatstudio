//! Integrity checks for optional, retained managed-install evidence.
//! Evidence is inventory only; it never grants distribution approval.
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{fs::File, io::Read, path::Path};

const MIB: u64 = 1024 * 1024;
fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn read(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let file = File::open(path).map_err(|e| format!("Missing managed evidence: {e}"))?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > limit {
        return Err("Managed evidence exceeds budget or is not a file".into());
    }
    let mut bytes = Vec::new();
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("Managed evidence exceeds budget".into());
    }
    Ok(bytes)
}
fn checked_json(bytes: &[u8]) -> Result<Value, String> {
    serde_json::from_slice(bytes).map_err(|e| format!("Invalid managed evidence JSON: {e}"))
}

pub fn validate(root: &Path, profile: &Value) -> Result<(), String> {
    let Some(reference) = profile.get("license_evidence") else {
        return Ok(());
    };
    // Python's legacy profiles may explicitly omit evidence using null.
    if reference.is_null() {
        return Ok(());
    }
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let expected = root.join("evidence/index.json");
    let path = Path::new(reference["path"].as_str().ok_or("Missing evidence path")?);
    if reference["schema_version"].as_u64() != Some(1) || !path.is_absolute() {
        return Err("Invalid managed evidence reference".into());
    }
    let path = path.canonicalize().map_err(|e| e.to_string())?;
    if path != expected || !path.starts_with(&root) {
        return Err("Managed evidence path escapes its directory".into());
    }
    let raw = read(&path, MIB)?;
    if reference["sha256"].as_str() != Some(digest(&raw).as_str()) {
        return Err("Managed evidence index hash mismatch".into());
    }
    let index = checked_json(&raw)?;
    let revision = profile["revision"]
        .as_str()
        .ok_or("Missing evidence revision")?;
    if index["schema_version"].as_u64() != Some(1)
        || index["revision"].as_str() != Some(revision)
        || index["distribution_approved"].as_bool() != Some(false)
    {
        return Err("Managed evidence revision/schema/approval mismatch".into());
    }
    let model_revision = index["setup_manifest"]["model_revision"]
        .as_str()
        .filter(|s| !s.is_empty())
        .ok_or("Missing setup manifest revision")?;
    if !revision.starts_with(&format!("{model_revision}:")) {
        return Err("Managed evidence manifest revision mismatch".into());
    }
    let files = index["files"]
        .as_array()
        .filter(|f| f.len() == 2)
        .ok_or("Managed evidence file set mismatch")?;
    let directory = path.parent().ok_or("Missing evidence directory")?;
    let mut freeze = None;
    let mut distributions = None;
    for item in files {
        let name = item["path"].as_str().ok_or("Missing evidence filename")?;
        let limit = match name {
            "freeze.txt" if freeze.is_none() => MIB,
            "distributions.json" if distributions.is_none() => 8 * MIB,
            _ => return Err("Managed evidence file set mismatch".into()),
        };
        let selected = directory
            .join(name)
            .canonicalize()
            .map_err(|e| e.to_string())?;
        if selected.parent() != Some(directory) {
            return Err("Managed evidence file escapes directory".into());
        }
        let bytes = read(&selected, limit)?;
        if item["size_bytes"].as_u64() != Some(bytes.len() as u64)
            || item["sha256"].as_str() != Some(digest(&bytes).as_str())
        {
            return Err("Managed evidence file size/hash mismatch".into());
        }
        if name == "freeze.txt" {
            freeze = Some(bytes);
        } else {
            distributions = Some(bytes);
        }
    }
    let freeze = freeze.ok_or("Missing freeze evidence")?;
    std::str::from_utf8(&freeze).map_err(|_| "Invalid freeze UTF-8")?;
    if !revision.ends_with(&format!(":{}", digest(&freeze))) {
        return Err("Managed freeze revision mismatch".into());
    }
    let distributions = checked_json(&distributions.ok_or("Missing distributions evidence")?)?;
    if distributions["schema_version"].as_u64() != Some(1)
        || distributions["installed"]
            .as_array()
            .is_none_or(|a| a.len() > 2000)
    {
        return Err("Invalid installed distribution evidence".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        fs,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture {
        root: PathBuf,
        profile: Value,
        index: Value,
    }
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "managed-evidence-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(root.join("evidence")).unwrap();
            let root = root.canonicalize().unwrap();
            let freeze = b"example==1.0\n";
            let distributions = br#"{"schema_version":1,"installed":[]}"#;
            fs::write(root.join("evidence/freeze.txt"), freeze).unwrap();
            fs::write(root.join("evidence/distributions.json"), distributions).unwrap();
            let revision = format!("model:{}", digest(freeze));
            let index = json!({"schema_version":1,"revision":revision,"distribution_approved":false,"setup_manifest":{"model_revision":"model"},"files":[{"path":"freeze.txt","size_bytes":freeze.len(),"sha256":digest(freeze)},{"path":"distributions.json","size_bytes":distributions.len(),"sha256":digest(distributions)}]});
            let profile = json!({"revision":revision,"license_evidence":{"schema_version":1,"path":root.join("evidence/index.json"),"sha256":""}});
            let mut result = Self {
                root,
                profile,
                index,
            };
            result.save();
            result
        }
        fn save(&mut self) {
            let bytes = serde_json::to_vec(&self.index).unwrap();
            fs::write(self.root.join("evidence/index.json"), &bytes).unwrap();
            self.profile["license_evidence"]["sha256"] = json!(digest(&bytes));
        }
        fn rejects(&self) {
            assert!(validate(&self.root, &self.profile).is_err());
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
    #[test]
    fn legacy_and_valid_profiles() {
        assert!(validate(Path::new("/does-not-exist"), &json!({})).is_ok());
        let f = Fixture::new();
        assert!(validate(&f.root, &f.profile).is_ok());
    }
    #[test]
    fn rejects_corruption_and_missing_files() {
        let mut f = Fixture::new();
        fs::write(f.root.join("evidence/freeze.txt"), b"altered").unwrap();
        f.rejects();
        f.index["files"][0]["size_bytes"] = json!(7);
        f.save();
        f.rejects();
        fs::remove_file(f.root.join("evidence/distributions.json")).unwrap();
        f.rejects();
    }
    #[test]
    fn rejects_index_tampering_revision_approval_and_extra_entries() {
        let mut f = Fixture::new();
        f.profile["license_evidence"]["sha256"] = json!("wrong");
        f.rejects();
        f.index["revision"] = json!("different");
        f.save();
        f.rejects();
        f.index["revision"] = f.profile["revision"].clone();
        f.index["distribution_approved"] = json!(true);
        f.save();
        f.rejects();
        f.index["distribution_approved"] = json!(false);
        f.index["files"]
            .as_array_mut()
            .unwrap()
            .push(json!({"path":"extra"}));
        f.save();
        f.rejects();
    }
    #[test]
    fn rejects_path_escape_and_freeze_revision_mismatch() {
        let mut f = Fixture::new();
        f.profile["license_evidence"]["path"] = json!("../index.json");
        f.rejects();
        f.profile["license_evidence"]["path"] = json!(f.root.join("evidence/index.json"));
        f.profile["revision"] = json!("model:wrong");
        f.index["revision"] = json!("model:wrong");
        f.save();
        f.rejects();
        f.index["files"][0]["path"] = json!("../freeze.txt");
        f.save();
        f.rejects();
    }
    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape() {
        use std::os::unix::fs::symlink;
        let f = Fixture::new();
        fs::write(f.root.join("outside"), b"example==1.0\n").unwrap();
        fs::remove_file(f.root.join("evidence/freeze.txt")).unwrap();
        symlink(f.root.join("outside"), f.root.join("evidence/freeze.txt")).unwrap();
        f.rejects();
    }
}
