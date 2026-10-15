//! Tiny original sound-check assets. These tones are never presented as speech.
use motion_lipsync::{Alignment, AlignmentSource, TimedPhoneme};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub struct DemoAssets {
    pub root: PathBuf,
    pub source: String,
    pub alignments: String,
}
impl DemoAssets {
    pub fn create() -> Result<Self, String> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos();
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = create_directory(&std::env::temp_dir(), nonce, &NEXT)?;
        let demo = Self {
            root,
            source: SOURCE.into(),
            alignments: illustrative_alignments()?,
        };
        for (name, frequency) in [("emma.wav", 440.0), ("alex.wav", 660.0)] {
            let samples = (0..12_000)
                .map(|i| {
                    let t = i as f32 / 24_000.0;
                    let envelope = (t / 0.02).min(1.0) * ((0.5 - t) / 0.02).min(1.0);
                    0.15 * envelope
                        * (std::f32::consts::TAU * (frequency * t + 100.0 * t * t)).sin()
                })
                .collect();
            motion_audio::write_wav(
                demo.root.join(name),
                &motion_audio::AudioBuffer {
                    sample_rate: 24_000,
                    channels: 1,
                    samples,
                },
            )
            .map_err(|e| e.to_string())?;
        }
        std::fs::write(demo.root.join("sound-check.yaml"), &demo.source)
            .map_err(|e| e.to_string())?;
        Ok(demo)
    }
}
// Exclusive creation retains ownership even when timestamps repeat or stale folders exist.
fn create_directory(parent: &Path, nonce: u128, next: &AtomicU64) -> Result<PathBuf, String> {
    for _ in 0..128 {
        let sequence = next.fetch_add(1, Ordering::Relaxed);
        let root = parent.join(format!(
            "kilsat-sound-check-{}-{nonce}-{sequence}",
            std::process::id()
        ));
        match std::fs::create_dir(&root) {
            Ok(()) => return Ok(root),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("Create sound-check directory: {error}")),
        }
    }
    Err(
        "Create sound-check directory: could not reserve a unique directory after 128 attempts"
            .into(),
    )
}

impl Drop for DemoAssets {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
fn illustrative_alignments() -> Result<String, String> {
    // Manual mouth poses for synthetic sound, not measured speech alignment.
    let alignment = Alignment {
        source: AlignmentSource::Manual,
        duration_seconds: 0.5,
        phonemes: vec![
            TimedPhoneme {
                phoneme: "ɑ".into(),
                start_seconds: 0.04,
                end_seconds: 0.22,
            },
            TimedPhoneme {
                phoneme: "u".into(),
                start_seconds: 0.24,
                end_seconds: 0.46,
            },
        ],
    };
    serde_json::to_string_pretty(&BTreeMap::from([
        (0u64, alignment.clone()),
        (1u64, alignment),
    ]))
    .map_err(|e| e.to_string())
}
pub fn runtime_path(executable: Option<&Path>) -> PathBuf {
    if let Some(contents) = executable.and_then(Path::parent).and_then(Path::parent) {
        let bundled = contents.join("Resources/services/kokoro-runtime/runtime.py");
        if bundled.is_file() {
            return bundled;
        }
    }
    PathBuf::from("services/kokoro-runtime/runtime.py")
}
const SOURCE: &str = r#"project:
  title: Synthetic sound check — not Kokoro speech
  fps: 24
  width: 1920
  height: 1080
characters:
  EMMA:
    asset: characters/emma
    voice: {provider: kokoro, preset: af_heart, language: en-us, speed: 1.0}
  ALEX:
    asset: characters/alex
    voice: {provider: kokoro, preset: am_michael, language: en-us, speed: 1.0}
scenes:
  - id: sound_check
    duration: 5.0
    events:
      - {type: move, at: 0.0, actor: EMMA, x: 500, y: 740, duration: 0.5}
      - {type: move, at: 0.0, actor: ALEX, x: 1400, y: 740, duration: 0.5}
      - {type: emotion, at: 0.5, actor: EMMA, value: happy}
      - {type: gesture, at: 0.5, actor: EMMA, value: wave, duration: 1.0}
      - {type: speak, at: 1.0, actor: EMMA, text: "Synthetic tone A; no speech", audio: emma.wav}
      - {type: emotion, at: 2.0, actor: ALEX, value: surprised}
      - {type: speak, at: 2.5, actor: ALEX, text: "Synthetic tone B; no speech", audio: alex.wav}
      - {type: move, at: 3.2, actor: EMMA, x: 850, y: 740, duration: 1.0}
"#;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn demo_is_small_importable_and_has_manual_cues() {
        let demo = DemoAssets::create().unwrap();
        let root = demo.root.clone();
        let script = motion_script::parse(&demo.source).unwrap();
        let jobs = motion_voice::dialogue_jobs(&script, &root).unwrap();
        let alignments: BTreeMap<u64, Alignment> = serde_json::from_str(&demo.alignments).unwrap();
        assert_eq!(jobs.len(), 2);
        for job in jobs {
            let audio = motion_audio::read_wav(job.imported_audio.unwrap()).unwrap();
            assert_eq!(
                (audio.sample_rate, audio.channels, audio.frames()),
                (24_000, 1, 12_000)
            );
            assert_eq!(
                audio.duration_seconds(),
                alignments[&job.id].duration_seconds
            );
            alignments[&job.id].validate().unwrap();
            assert!(matches!(
                alignments[&job.id].source,
                AlignmentSource::Manual
            ));
        }
        drop(demo);
        assert!(!root.exists());
    }
    #[test]
    fn collision_is_retried_without_reusing_or_removing_existing_assets() {
        let owner = DemoAssets::create().unwrap();
        let next = AtomicU64::new(0);
        let occupied = create_directory(&owner.root, 42, &next).unwrap();
        std::fs::write(occupied.join("owned"), b"keep").unwrap();
        next.store(0, Ordering::Relaxed);
        let fresh = create_directory(&owner.root, 42, &next).unwrap();
        assert_ne!(occupied, fresh);
        assert_eq!(std::fs::read(occupied.join("owned")).unwrap(), b"keep");
    }
    #[test]
    fn simultaneous_demos_have_independent_lifetimes() {
        let handles: Vec<_> = (0..8)
            .map(|_| std::thread::spawn(DemoAssets::create))
            .collect();
        let mut demos: Vec<_> = handles
            .into_iter()
            .map(|h| h.join().unwrap().unwrap())
            .collect();
        let paths: std::collections::BTreeSet<_> = demos.iter().map(|d| d.root.clone()).collect();
        assert_eq!(paths.len(), demos.len());
        let removed = demos.pop().unwrap();
        let removed_path = removed.root.clone();
        drop(removed);
        assert!(!removed_path.exists());
        for demo in &demos {
            assert!(demo.root.join("emma.wav").is_file());
        }
        let recreated = DemoAssets::create().unwrap();
        assert!(!paths.contains(&recreated.root));
    }
    #[test]
    fn bundled_runtime_is_resolved_without_working_directory() {
        let demo = DemoAssets::create().unwrap();
        let contents = demo.root.join("Kilsat Studio.app/Contents");
        let runtime = contents.join("Resources/services/kokoro-runtime/runtime.py");
        std::fs::create_dir_all(runtime.parent().unwrap()).unwrap();
        std::fs::write(&runtime, "# packaged sidecar").unwrap();
        let executable = contents.join("MacOS/studio-desktop");
        assert_eq!(runtime_path(Some(&executable)), runtime);
    }
    #[test]
    fn nonbundle_runtime_uses_source_checkout_path() {
        assert_eq!(
            runtime_path(None),
            PathBuf::from("services/kokoro-runtime/runtime.py")
        );
        assert_eq!(
            runtime_path(Some(Path::new("/nonexistent/kilsat"))),
            runtime_path(None)
        );
    }
}
