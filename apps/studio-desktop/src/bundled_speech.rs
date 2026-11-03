//! Speech engines shipped inside the app (`Contents/Resources/speech`).
//!
//! `scripts/build_speech_bundle.py` writes `speech-bundle.json` with paths relative
//! to the bundle. At startup the desktop resolves them, so Kokoro, Piper and Rhubarb
//! work without any download or setup. Writable state (caches and the generated
//! Piper profile) lives in Application Support, never inside the signed app.
use serde::Deserialize;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

pub const DESCRIPTOR: &str = "speech-bundle.json";
/// Developer override for running an unpackaged build against a built bundle.
pub const ENV_OVERRIDE: &str = "KILSAT_SPEECH_BUNDLE";
/// eSpeak NG ignores data paths of 160 characters or more; keep a margin for file names.
const MAX_ESPEAK_PATH: usize = 150;

#[derive(Deserialize)]
struct Descriptor {
    schema_version: u32,
    bundle_id: String,
    kokoro: KokoroEntry,
    piper: PiperEntry,
    rhubarb: RhubarbEntry,
}
#[derive(Deserialize)]
struct KokoroEntry {
    python: String,
    config: String,
    model: String,
    voices: String,
    revision: String,
}
#[derive(Deserialize)]
struct PiperEntry {
    executable: String,
    model: String,
    config: String,
    preset: String,
    model_version: String,
    environment_identity: String,
}
#[derive(Deserialize)]
struct RhubarbEntry {
    executable: String,
    version: String,
}

/// Absolute, existing paths of every bundled engine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundledSpeech {
    pub bundle_id: String,
    pub python: PathBuf,
    pub kokoro_config: PathBuf,
    pub kokoro_model: PathBuf,
    pub kokoro_voices: PathBuf,
    pub kokoro_revision: String,
    pub kokoro_cache: PathBuf,
    pub piper_profile: PathBuf,
    pub rhubarb: PathBuf,
    pub rhubarb_version: String,
}

/// The bundle next to the running executable, or the developer override.
pub fn locate(executable: Option<&Path>, override_dir: Option<PathBuf>) -> Option<PathBuf> {
    let bundled = executable
        .and_then(Path::parent)
        .and_then(Path::parent)
        .map(|contents| contents.join("Resources/speech"));
    bundled
        .into_iter()
        .chain(override_dir.filter(|p| p.is_absolute()))
        .find(|root| root.join(DESCRIPTOR).is_file())
}

/// Writable per-user folder for bundled-speech caches and generated profiles.
pub fn data_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("Library/Application Support/Kilsat Studio/Bundled speech")
}

fn inside(root: &Path, relative: &str, directory: bool) -> Result<PathBuf, String> {
    let path = Path::new(relative);
    if relative.is_empty() || !path.components().all(|c| matches!(c, Component::Normal(_))) {
        return Err(format!("Puhepaketin polku ei ole turvallinen: {relative}"));
    }
    let joined = root.join(path);
    let resolved = joined
        .canonicalize()
        .map_err(|e| format!("Puhepaketista puuttuu {relative}: {e}"))?;
    let ok = resolved.starts_with(root)
        && if directory {
            resolved.is_dir()
        } else {
            resolved.is_file()
        };
    if !ok {
        return Err(format!("Puhepaketin tiedosto on virheellinen: {relative}"));
    }
    Ok(joined)
}

/// The bundled Python runtime and Kokoro are arm64-only builds
/// (`scripts/build_speech_bundle.py`); an Intel Mac cannot run them.
fn require_apple_silicon(arch: &str) -> Result<(), String> {
    if arch == "aarch64" {
        return Ok(());
    }
    Err("Mukana tulevat puhemoottorit toimivat vain Apple Silicon -Maceissa (M1 tai uudempi). \
         Tällä Macilla voit tuoda puheen äänitiedostona tai määrittää oman paikallisen puhemoottorin."
        .into())
}

/// Resolve the bundle and write the Piper profile that points at it.
pub fn load(root: &Path, data: &Path) -> Result<BundledSpeech, String> {
    require_apple_silicon(std::env::consts::ARCH)?;
    let root = root
        .canonicalize()
        .map_err(|e| format!("Puhepaketti: {e}"))?;
    let raw = fs::read(root.join(DESCRIPTOR)).map_err(|e| format!("Puhepaketti: {e}"))?;
    if raw.len() > 1024 * 1024 {
        return Err("Puhepaketin kuvaus on yli 1 MiB".into());
    }
    let espeak = root.join("espeak/kokoro");
    if espeak.as_os_str().len() > MAX_ESPEAK_PATH {
        return Err(format!(
            "Sovelluksen kansiopolku on liian pitkä puhemoottorille ({} merkkiä, enintään {MAX_ESPEAK_PATH}). Siirrä Koeta Studio Ohjelmat-kansioon.",
            espeak.as_os_str().len()
        ));
    }
    let d: Descriptor =
        serde_json::from_slice(&raw).map_err(|e| format!("Puhepaketin kuvaus: {e}"))?;
    if d.schema_version != 1 {
        return Err(format!(
            "Puhepaketin versiota {} ei tueta",
            d.schema_version
        ));
    }
    let labels = [
        &d.bundle_id,
        &d.kokoro.revision,
        &d.piper.model_version,
        &d.piper.environment_identity,
        &d.rhubarb.version,
    ];
    if labels.iter().any(|s| s.trim().is_empty() || s.len() > 4096) {
        return Err("Puhepaketin versiotiedot puuttuvat".into());
    }
    let kokoro_cache = data.join("kokoro-cache");
    let piper_cache = data.join("piper-cache");
    for directory in [&kokoro_cache, &piper_cache] {
        fs::create_dir_all(directory).map_err(|e| format!("{}: {e}", directory.display()))?;
    }
    let profile = motion_tts::PiperConfig {
        executable: inside(&root, &d.piper.executable, false)?,
        model_path: inside(&root, &d.piper.model, false)?,
        config_path: inside(&root, &d.piper.config, false)?,
        cache_directory: piper_cache,
        preset: d.piper.preset,
        model_version: d.piper.model_version,
        environment_identity: d.piper.environment_identity,
        timeout_seconds: 180,
    };
    let piper_profile = data.join("piper-profile.json");
    write_if_changed(
        &piper_profile,
        &serde_json::to_vec_pretty(&profile).map_err(|e| e.to_string())?,
    )?;
    Ok(BundledSpeech {
        bundle_id: d.bundle_id,
        python: inside(&root, &d.kokoro.python, false)?,
        kokoro_config: inside(&root, &d.kokoro.config, false)?,
        kokoro_model: inside(&root, &d.kokoro.model, false)?,
        kokoro_voices: inside(&root, &d.kokoro.voices, true)?,
        kokoro_revision: d.kokoro.revision,
        kokoro_cache,
        piper_profile,
        rhubarb: inside(&root, &d.rhubarb.executable, false)?,
        rhubarb_version: d.rhubarb.version,
    })
}

fn write_if_changed(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if fs::read(path).is_ok_and(|current| current == bytes) {
        return Ok(());
    }
    let pending = path.with_extension("json.tmp");
    fs::write(&pending, bytes)
        .and_then(|()| fs::rename(&pending, path))
        .map_err(|e| format!("{}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempDir(PathBuf);
    impl TempDir {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "kilsat-bundled-speech-{name}-{}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&path);
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn bundle(root: &Path) {
        for file in [
            "python/bin/python3.12",
            "kokoro/config.json",
            "kokoro/kokoro-v1_0.pth",
            "kokoro/voices/af_heart.pt",
            "bin/piper",
            "piper/fi_FI-harri-medium.onnx",
            "piper/fi_FI-harri-medium.onnx.json",
            "rhubarb/rhubarb",
        ] {
            let path = root.join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, b"x").unwrap();
        }
        descriptor(root, "bin/piper");
    }
    fn descriptor(root: &Path, piper: &str) {
        let text = serde_json::json!({
            "schema_version": 1, "bundle_id": "test", "files": [],
            "kokoro": {"python": "python/bin/python3.12", "config": "kokoro/config.json",
                       "model": "kokoro/kokoro-v1_0.pth", "voices": "kokoro/voices", "revision": "rev:abc"},
            "piper": {"executable": piper, "model": "piper/fi_FI-harri-medium.onnx",
                      "config": "piper/fi_FI-harri-medium.onnx.json", "preset": "piper_harri",
                      "model_version": "voices@rev", "environment_identity": "env"},
            "rhubarb": {"executable": "rhubarb/rhubarb", "version": "1.14.0", "architecture": "x86_64"}
        });
        fs::write(root.join(DESCRIPTOR), text.to_string()).unwrap();
    }

    #[test]
    fn resolves_every_engine_and_writes_an_absolute_piper_profile() {
        let temp = TempDir::new("resolve");
        let (root, data) = (temp.path().join("speech"), temp.path().join("data"));
        fs::create_dir(&root).unwrap();
        bundle(&root);
        let speech = load(&root, &data).unwrap();
        let root = root.canonicalize().unwrap();
        assert_eq!(speech.python, root.join("python/bin/python3.12"));
        assert_eq!(speech.kokoro_revision, "rev:abc");
        assert_eq!(speech.rhubarb, root.join("rhubarb/rhubarb"));
        assert!(speech.kokoro_cache.is_dir());
        let profile: motion_tts::PiperConfig =
            serde_json::from_slice(&fs::read(&speech.piper_profile).unwrap()).unwrap();
        assert_eq!(profile.executable, root.join("bin/piper"));
        assert_eq!(profile.preset, "piper_harri");
        assert!(profile.cache_directory.starts_with(&data));
        // Loading again leaves an identical profile untouched.
        let before = fs::metadata(&speech.piper_profile)
            .unwrap()
            .modified()
            .unwrap();
        load(&root, &data).unwrap();
        let after = fs::metadata(&speech.piper_profile)
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn escaping_absolute_or_missing_paths_are_rejected() {
        let temp = TempDir::new("escape");
        let root = temp.path().join("speech");
        fs::create_dir(&root).unwrap();
        bundle(&root);
        fs::write(temp.path().join("outside"), b"x").unwrap();
        for piper in ["../outside", "/bin/sh", "bin/missing", "bin"] {
            descriptor(&root, piper);
            assert!(load(&root, &temp.path().join("data")).is_err(), "{piper}");
        }
    }

    #[test]
    fn too_long_install_paths_are_reported_before_speech_fails() {
        let temp = TempDir::new("long");
        let root = temp.path().join("x".repeat(150)).join("speech");
        fs::create_dir_all(&root).unwrap();
        bundle(&root);
        let error = load(&root, &temp.path().join("data")).unwrap_err();
        assert!(error.contains("Ohjelmat-kansioon"), "{error}");
    }

    #[test]
    fn intel_macs_get_a_clear_error_instead_of_launching_arm64_engines() {
        assert!(require_apple_silicon("aarch64").is_ok());
        let error = require_apple_silicon("x86_64").unwrap_err();
        assert!(error.contains("Apple Silicon") && error.contains("äänitiedostona"));
    }

    #[test]
    fn locate_prefers_the_app_bundle_and_ignores_relative_overrides() {
        let temp = TempDir::new("locate");
        let contents = temp.path().join("Koeta Studio.app/Contents");
        let speech = contents.join("Resources/speech");
        fs::create_dir_all(&speech).unwrap();
        let executable = contents.join("MacOS/studio-desktop");
        assert_eq!(locate(Some(&executable), None), None);
        fs::write(speech.join(DESCRIPTOR), b"{}").unwrap();
        assert_eq!(locate(Some(&executable), None), Some(speech.clone()));
        assert_eq!(locate(None, Some("relative".into())), None);
        assert_eq!(locate(None, Some(speech.clone())), Some(speech));
    }
}
