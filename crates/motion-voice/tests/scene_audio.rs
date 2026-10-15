use motion_audio::{write_wav, AudioBuffer};
use motion_lipsync::{Alignment, AlignmentSource, TimedPhoneme, Viseme};
use motion_tts::{SpeechError, SpeechProvider, SpeechRequest, SpeechResult};
use motion_voice::scene_audio::prepare_scene;
use std::{
    collections::BTreeMap,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

struct Fixture {
    path: PathBuf,
    calls: AtomicUsize,
    fail: bool,
}
impl Fixture {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "kilsat-scene-{}-{}.wav",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        write_wav(
            &path,
            &AudioBuffer {
                sample_rate: 24000,
                channels: 1,
                samples: vec![0.25; 24],
            },
        )
        .unwrap();
        Self {
            path,
            calls: AtomicUsize::new(0),
            fail: false,
        }
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}
impl SpeechProvider for Fixture {
    fn synthesize(&self, _: SpeechRequest) -> Result<SpeechResult, SpeechError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.fail {
            return Err(SpeechError("Kokoro unavailable".into()));
        }
        Ok(SpeechResult {
            audio_path: self.path.clone(),
            sample_rate: 24000,
            sample_count: 24,
            cache_key: String::new(),
            phonemes: "m".into(),
            cached: false,
        })
    }
}
fn script() -> motion_script::Script {
    motion_script::parse(
        r#"
characters:
  EMMA:
    asset: characters/emma
    voice: { provider: kokoro, preset: af_heart, language: en-us, speed: 1.0 }
  ALEX:
    asset: characters/alex
    voice: { provider: kokoro, preset: am_michael, language: en-us, speed: 1.0 }
scenes:
  - id: previous
    duration: 1.0
    events:
      - { type: speak, at: 0.0, actor: EMMA, text: Previous }
  - id: selected
    duration: 0.01
    events:
      - { type: speak, at: 0.001, actor: EMMA, text: One }
      - { type: speak, at: 0.0015, actor: ALEX, text: Two }
  - id: silent
    duration: 0.001
    events: []
"#,
    )
    .unwrap()
}
fn alignment() -> Alignment {
    Alignment {
        source: AlignmentSource::Manual,
        duration_seconds: 0.001,
        phonemes: vec![TimedPhoneme {
            phoneme: "m".into(),
            start_seconds: 0.0,
            end_seconds: 0.0005,
        }],
    }
}
#[test]
fn synthesizes_only_selected_scene_and_mixes_on_shared_clock() {
    let provider = Fixture::new();
    let mut progress = Vec::new();
    let prepared = prepare_scene(
        &script(),
        1,
        std::path::Path::new("."),
        &provider,
        &BTreeMap::from([(1, alignment())]),
        |p| progress.push(p),
    )
    .unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 2);
    assert_eq!(prepared.scene_id, "selected");
    assert_eq!(
        progress
            .iter()
            .map(|p| (p.dialogue_id, p.completed, p.total))
            .collect::<Vec<_>>(),
        [(1, 1, 2), (2, 2, 2)]
    );
    let audio = &prepared.rendered.audio.samples;
    assert_eq!(audio[23], 0.0);
    assert!((audio[24] - 0.25).abs() < 0.001);
    assert!((audio[36] - 0.5).abs() < 0.001);
    assert_eq!(audio[60], 0.0);
    let track = &prepared.rendered.manifest.dialogue[0];
    assert_eq!(track.visemes[0].start_frame, 24);
    assert_eq!(track.visemes[0].end_frame, 36);
    assert_eq!(prepared.clips[0].mouth_at(24).unwrap(), Viseme::Closed);
    assert_eq!(prepared.clips[0].mouth_at(36).unwrap(), Viseme::Rest);
    assert!(prepared.clips[1].alignment.is_none());
    assert!(prepared.rendered.manifest.dialogue[1].visemes.is_empty());
}
#[test]
fn imports_explicit_audio_without_calling_provider() {
    let mut provider = Fixture::new();
    provider.fail = true;
    let mut script = script();
    let name = provider
        .path
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    if let motion_script::Event::Speak { audio, .. } = &mut script.scenes[1].events[0] {
        *audio = Some(name);
    }
    script.scenes[1].events.truncate(1);
    let prepared = prepare_scene(
        &script,
        1,
        provider.path.parent().unwrap(),
        &provider,
        &BTreeMap::from([(1, alignment())]),
        |_| {},
    )
    .unwrap();
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    assert_eq!(prepared.clips[0].mouth_at(24).unwrap(), Viseme::Closed);
}
#[test]
fn silence_and_unknown_alignment_are_explicit() {
    let provider = Fixture::new();
    let silent = prepare_scene(
        &script(),
        2,
        std::path::Path::new("."),
        &provider,
        &BTreeMap::new(),
        |_| panic!("no dialogue progress"),
    )
    .unwrap();
    assert_eq!(silent.rendered.audio.samples, vec![0.0; 24]);
    assert!(silent.clips.is_empty());
    let error = prepare_scene(
        &script(),
        1,
        std::path::Path::new("."),
        &provider,
        &BTreeMap::from([(0, alignment())]),
        |_| {},
    )
    .err()
    .unwrap();
    assert!(error
        .to_string()
        .contains("does not belong to scene selected"));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
}
#[test]
fn reports_failure_alignment_mismatch_and_overrun() {
    let mut provider = Fixture::new();
    provider.fail = true;
    let error = prepare_scene(
        &script(),
        1,
        std::path::Path::new("."),
        &provider,
        &BTreeMap::new(),
        |_| {},
    )
    .err()
    .unwrap();
    assert!(error
        .to_string()
        .contains("scene selected dialogue 1 (EMMA): Kokoro unavailable"));
    provider.fail = false;
    let mut wrong = alignment();
    wrong.duration_seconds = 0.002;
    let error = prepare_scene(
        &script(),
        1,
        std::path::Path::new("."),
        &provider,
        &BTreeMap::from([(1, wrong)]),
        |_| {},
    )
    .err()
    .unwrap();
    assert!(error
        .to_string()
        .contains("alignment duration differs from audio"));
    let mut script = script();
    script.scenes[1].duration = 0.002;
    let error = prepare_scene(
        &script,
        1,
        std::path::Path::new("."),
        &provider,
        &BTreeMap::new(),
        |_| {},
    )
    .err()
    .unwrap();
    assert!(error.to_string().contains("exceeds scene"));
}

#[test]
fn rejects_oversized_scene_before_inference_and_same_actor_overlap() {
    let provider = Fixture::new();
    let mut screenplay = script();
    screenplay.scenes[1].duration = 3600.1;
    assert!(prepare_scene(
        &screenplay,
        1,
        std::path::Path::new("."),
        &provider,
        &BTreeMap::new(),
        |_| {}
    )
    .is_err());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);
    screenplay.scenes[1].duration = 0.01;
    if let motion_script::Event::Speak { actor, .. } = &mut screenplay.scenes[1].events[1] {
        *actor = "EMMA".into();
    }
    let error = prepare_scene(
        &screenplay,
        1,
        std::path::Path::new("."),
        &provider,
        &BTreeMap::new(),
        |_| {},
    )
    .err()
    .unwrap();
    assert!(error
        .to_string()
        .contains("overlapping dialogue for the same actor"));
}
