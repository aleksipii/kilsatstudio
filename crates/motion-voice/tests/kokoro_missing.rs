//! Acceptance test: with Kokoro not installed, an English line without imported
//! audio fails with an error that names the missing asset and the offline fallback,
//! and the same line with imported audio prepares through the same real provider.
use motion_audio::{write_wav, AudioBuffer};
use motion_tts::{KokoroConfig, LocalSpeechProvider};
use motion_voice::scene_audio::prepare_scene;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

struct Root(PathBuf);
impl Root {
    fn new(tag: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "kilsat-kokoro-missing-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(path.join("audio")).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A Kokoro install whose files do not exist (nothing is created or downloaded).
fn missing_kokoro(root: &Root) -> LocalSpeechProvider {
    let absent = root.0.join("not-installed");
    LocalSpeechProvider::new(KokoroConfig {
        python: absent.join("bin/python3"),
        runtime_script: absent.join("kokoro_runtime.py"),
        model_config: absent.join("config.json"),
        model_weights: absent.join("kokoro-v1_0.pth"),
        voices_directory: absent.join("voices"),
        cache_directory: root.0.join("cache"),
        model_version: "fixture".into(),
        timeout: Duration::from_secs(5),
    })
}

fn script(audio: &str) -> motion_script::Script {
    motion_script::parse(&format!(
        "characters:
  EMMA: {{asset: characters/emma, voice: {{provider: kokoro, preset: af_heart, language: en-us, speed: 1.0}}}}
scenes:
  - id: kahvila
    duration: 1.5
    events:
      - {{type: speak, at: 0.25, actor: EMMA, text: \"Good night.\"{audio}}}
"
    ))
    .unwrap()
}

#[test]
fn missing_kokoro_names_the_asset_and_the_fallback() {
    let root = Root::new("error");
    let provider = missing_kokoro(&root);
    let Err(error) = prepare_scene(&script(""), 0, &root.0, &provider, &BTreeMap::new(), |_| {})
    else {
        panic!("a line without audio must not prepare when Kokoro is missing");
    };
    let error = error.to_string();
    for expected in [
        "scene kahvila",
        "EMMA",
        "Kokoro model config not found",
        "config.json",
        "import a 24 kHz mono WAV",
    ] {
        assert!(
            error.contains(expected),
            "{expected:?} missing from: {error}"
        );
    }
    // Nothing was synthesized or cached as a substitute.
    let cached = std::fs::read_dir(root.0.join("cache"))
        .map(|entries| entries.count())
        .unwrap_or(0);
    assert_eq!(cached, 0);
}

#[test]
fn imported_audio_works_without_kokoro() {
    let root = Root::new("imported");
    let samples = (0..12_000)
        .map(|i| 0.3 * (i as f32 * 200.0 * std::f32::consts::TAU / 24_000.0).sin())
        .collect();
    write_wav(
        root.0.join("audio/emma.wav"),
        &AudioBuffer {
            sample_rate: 24_000,
            channels: 1,
            samples,
        },
    )
    .unwrap();
    let provider = missing_kokoro(&root);
    let prepared = prepare_scene(
        &script(", audio: audio/emma.wav"),
        0,
        &root.0,
        &provider,
        &BTreeMap::new(),
        |_| {},
    )
    .unwrap();
    assert_eq!(prepared.clips.len(), 1);
    assert_eq!(prepared.clips[0].audio.frames(), 12_000);
}
