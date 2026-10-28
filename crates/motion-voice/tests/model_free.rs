//! Acceptance test for the strict model-free path (ADR-A01-09): a provider whose every
//! call fails and is counted is never called by the imported-WAV workflow, including
//! project save/reopen and export; missing audio is a typed error before any call.
use motion_audio::{read_wav, write_wav, AudioBuffer};
use motion_lipsync::{
    audio_timeline::{AudioSourceInfo, AudioTimeline},
    Alignment, AlignmentSource, TimedPhoneme, Viseme,
};
use motion_project::{ModelBackedStep, ModelPolicy, ProjectDocument};
use motion_tts::{CancellationToken, SpeechError, SpeechProvider, SpeechRequest, SpeechResult};
use motion_voice::{
    audio_timeline::scene_audio_timeline,
    model_free::{
        prepare_scene_model_free, require_imported_audio, MissingAudioReason, ModelFreeError,
    },
    scene_audio::prepare_scene_with_cancellation,
};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

/// Every call fails; calls are counted so the test can prove none happened.
#[derive(Default)]
struct FailingProvider {
    calls: AtomicUsize,
}
impl SpeechProvider for FailingProvider {
    fn synthesize(&self, _: SpeechRequest) -> Result<SpeechResult, SpeechError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Err(SpeechError("test provider: every call fails".into()))
    }
    fn synthesize_cancellable(
        &self,
        request: SpeechRequest,
        _: &CancellationToken,
    ) -> Result<SpeechResult, SpeechError> {
        self.synthesize(request)
    }
}

struct Root(PathBuf);
impl Root {
    fn new(tag: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "kilsat-model-free-{tag}-{}-{}",
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

/// Procedural test signal (a tone after leading silence), not speech.
fn write_signal(path: &Path, frames: usize, hz: f32) {
    let samples = (0..frames)
        .map(|i| {
            if i < 1200 {
                0.0
            } else {
                0.3 * (i as f32 * hz * std::f32::consts::TAU / 24_000.0).sin()
            }
        })
        .collect();
    write_wav(
        path,
        &AudioBuffer {
            sample_rate: 24_000,
            channels: 1,
            samples,
        },
    )
    .unwrap();
}

const SCRIPT: &str = "characters:
  AINO: {asset: characters/aino, voice: {provider: piper, preset: piper_fi_fixture, language: fi-fi, speed: 1.0}}
  EMMA: {asset: characters/emma, voice: {provider: kokoro, preset: af_heart, language: en-us, speed: 1.0}}
scenes:
  - id: kahvila
    duration: 2.0
    events:
      - {type: speak, at: 0.25, actor: AINO, text: \"Hyvää yötä, Emma.\", audio: audio/aino.wav}
      - {type: speak, at: 1.0, actor: EMMA, text: \"Good night.\", audio: audio/emma.wav}
";

/// Hand-authored timing for AINO's line: "m" closes the lips, then "ɑ".
fn manual_alignment() -> Alignment {
    Alignment {
        source: AlignmentSource::Manual,
        duration_seconds: 0.5,
        phonemes: vec![
            TimedPhoneme {
                phoneme: "m".into(),
                start_seconds: 0.05,
                end_seconds: 0.1,
            },
            TimedPhoneme {
                phoneme: "ɑ".into(),
                start_seconds: 0.1,
                end_seconds: 0.3,
            },
        ],
    }
}

#[test]
fn strict_wav_workflow_never_calls_the_provider_through_reopen_and_export() {
    let root = Root::new("workflow");
    write_signal(&root.0.join("audio/aino.wav"), 12_000, 180.0);
    write_signal(&root.0.join("audio/emma.wav"), 12_000, 220.0);
    let provider = FailingProvider::default();
    let policy = ModelPolicy::Strict;
    assert!(policy.permit(ModelBackedStep::SpeechSynthesis).is_err());
    assert!(policy.permit(ModelBackedStep::RecognizerAlignment).is_err());

    // 1. Prepare: strict check first, then the ordinary preparation with the failing provider.
    let script = motion_script::parse(SCRIPT).unwrap();
    assert_eq!(
        require_imported_audio(&script, Some(0), &root.0)
            .unwrap()
            .len(),
        2
    );
    let alignments = BTreeMap::from([(0, manual_alignment())]);
    let cancel = CancellationToken::new();
    let prepared = prepare_scene_with_cancellation(
        &script,
        0,
        &root.0,
        &provider,
        &alignments,
        &cancel,
        |_| {},
    )
    .unwrap();
    let strict =
        prepare_scene_model_free(&script, 0, &root.0, &alignments, &cancel, |_| {}).unwrap();
    assert_eq!(
        strict.rendered.audio.samples,
        prepared.rendered.audio.samples
    );
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);

    // 2. AudioTimeline v1: imported sources, Manual and rule-estimated mouth provenance.
    let timeline = scene_audio_timeline(&script, &root.0, &prepared, &BTreeMap::new()).unwrap();
    assert_eq!(timeline.format_version, 1);
    assert_eq!(timeline.duration_samples, 48_000);
    let [aino, emma] = &timeline.clips[..] else {
        panic!("two clips")
    };
    assert!(
        matches!(&aino.source, AudioSourceInfo::Imported { path, .. } if path == "audio/aino.wav")
    );
    assert_eq!((aino.start_sample, emma.start_sample), (6_000, 24_000));
    assert_eq!(aino.language, "fi-fi");
    assert_eq!(aino.mouth.as_ref().unwrap().source, AlignmentSource::Manual);
    assert_eq!(aino.mouth.as_ref().unwrap().phonemes.len(), 2);
    assert!(matches!(
        &emma.mouth.as_ref().unwrap().source,
        AlignmentSource::Estimated { tool, .. } if tool == "kilsat-english-estimator"
    ));
    // Manual cue boundaries: local 1200 → scene 7200 closes the lips; 2400 → A.
    assert_eq!(timeline.viseme_at("AINO", 7_199), Viseme::Rest);
    assert_eq!(timeline.viseme_at("AINO", 7_200), Viseme::Closed);
    assert_eq!(timeline.viseme_at("AINO", 8_400), Viseme::A);
    assert_eq!(timeline.viseme_at("AINO", 13_200), Viseme::Rest);

    // 3. Export: mixed WAV, manifest and timeline written; provenance survives.
    let export = root.0.join("export");
    std::fs::create_dir_all(&export).unwrap();
    write_wav(export.join("kahvila.wav"), &prepared.rendered.audio).unwrap();
    std::fs::write(
        export.join("kahvila.manifest.json"),
        serde_json::to_vec_pretty(&prepared.rendered.manifest).unwrap(),
    )
    .unwrap();
    std::fs::write(
        export.join("kahvila.audio-timeline.json"),
        timeline.to_json_pretty().unwrap(),
    )
    .unwrap();
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(export.join("kahvila.manifest.json")).unwrap())
            .unwrap();
    assert_eq!(
        manifest["dialogue"][0]["alignment_source"]["kind"],
        "manual"
    );
    assert_eq!(
        manifest["dialogue"][1]["alignment_source"]["kind"],
        "estimated"
    );

    // 4. Save and reopen the project, reload the timeline, prepare again.
    let project_path = root.0.join("scene.kilsat.json");
    motion_project::save_atomic(&project_path, &ProjectDocument::new(SCRIPT).unwrap()).unwrap();
    let reopened = motion_project::load(&project_path).unwrap();
    let script = motion_script::parse(&reopened.source).unwrap();
    let reloaded = AudioTimeline::from_json(
        &std::fs::read(export.join("kahvila.audio-timeline.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(reloaded, timeline);
    require_imported_audio(&script, None, &root.0).unwrap();
    let again = prepare_scene_with_cancellation(
        &script,
        0,
        &root.0,
        &provider,
        &alignments,
        &cancel,
        |_| {},
    )
    .unwrap();
    assert_eq!(
        scene_audio_timeline(&script, &root.0, &again, &BTreeMap::new()).unwrap(),
        timeline
    );

    // 5. The timeline alone reproduces the exported mix on the same clock and mixer.
    let clips: Vec<_> = reloaded
        .clips
        .iter()
        .map(|clip| {
            let AudioSourceInfo::Imported { path, pcm_sha256 } = &clip.source else {
                panic!()
            };
            let audio = read_wav(root.0.join(path)).unwrap();
            assert_eq!(
                &motion_lipsync::rhubarb::pcm_identity(&audio).unwrap(),
                pcm_sha256
            );
            clip.mix_clip(audio).unwrap()
        })
        .collect();
    let remixed =
        motion_audio::mix_timeline(&clips, 24_000, 1, reloaded.duration_samples as usize).unwrap();
    let exported = read_wav(export.join("kahvila.wav")).unwrap();
    assert_eq!(remixed.samples.len(), exported.samples.len());
    assert_eq!(remixed.samples, again.rendered.audio.samples);

    assert_eq!(
        provider.calls.load(Ordering::SeqCst),
        0,
        "strict path called the provider"
    );
}

#[test]
fn missing_wav_is_a_typed_error_before_any_provider_call() {
    let root = Root::new("missing");
    let source = SCRIPT
        .replace(", audio: audio/aino.wav", "")
        .replace("audio/emma.wav", "audio/puuttuu.wav");
    let script = motion_script::parse(&source).unwrap();
    let provider = FailingProvider::default();

    let Err(ModelFreeError::MissingImportedAudio(missing)) =
        require_imported_audio(&script, Some(0), &root.0)
    else {
        panic!("expected missing audio");
    };
    assert_eq!(missing.len(), 2);
    assert_eq!(
        (missing[0].dialogue_id, missing[0].actor.as_str()),
        (0, "AINO")
    );
    assert_eq!(missing[0].reason, MissingAudioReason::NotSpecified);
    assert_eq!(
        missing[1].reason,
        MissingAudioReason::FileMissing(root.0.join("audio/puuttuu.wav"))
    );
    let error = prepare_scene_model_free(
        &script,
        0,
        &root.0,
        &BTreeMap::new(),
        &CancellationToken::new(),
        |_| {},
    )
    .err()
    .unwrap();
    assert!(matches!(error, ModelFreeError::MissingImportedAudio(ref lines) if lines.len() == 2));
    assert!(error.to_string().contains("dialogue 0 (AINO) has no audio"));
    assert_eq!(provider.calls.load(Ordering::SeqCst), 0);

    // Without the strict check the ordinary path would ask the provider (and fail):
    // the check, not luck, is what keeps the strict path model-free. No fallback runs.
    let result = prepare_scene_with_cancellation(
        &script,
        0,
        &root.0,
        &provider,
        &BTreeMap::new(),
        &CancellationToken::new(),
        |_| {},
    );
    assert!(result.is_err());
    assert_eq!(provider.calls.load(Ordering::SeqCst), 1);
}

#[test]
fn synthesized_audio_without_recorded_origin_is_rejected() {
    let root = Root::new("origin");
    write_signal(&root.0.join("audio/aino.wav"), 12_000, 180.0);
    let source = SCRIPT.replace(", audio: audio/emma.wav", "");
    let script = motion_script::parse(&source).unwrap();
    // A provider that "synthesizes" by returning the fixture file.
    struct Fake(PathBuf);
    impl SpeechProvider for Fake {
        fn synthesize(&self, _: SpeechRequest) -> Result<SpeechResult, SpeechError> {
            Ok(SpeechResult {
                audio_path: self.0.clone(),
                sample_rate: 24_000,
                sample_count: 12_000,
                cache_key: "cache".into(),
                phonemes: String::new(),
                cached: false,
            })
        }
    }
    assert!(ModelPolicy::LocalPreprocessing
        .permit(ModelBackedStep::SpeechSynthesis)
        .is_ok());
    let provider = Fake(root.0.join("audio/aino.wav"));
    let prepared = prepare_scene_with_cancellation(
        &script,
        0,
        &root.0,
        &provider,
        &BTreeMap::new(),
        &CancellationToken::new(),
        |_| {},
    )
    .unwrap();
    let error = scene_audio_timeline(&script, &root.0, &prepared, &BTreeMap::new()).unwrap_err();
    assert!(error.0.contains("no recorded provider origin"), "{error}");
    let origins = BTreeMap::from([(
        1,
        motion_voice::audio_timeline::SynthesisOrigin {
            provider: "kokoro".into(),
            model_revision: "fixture-revision".into(),
            voice_revision: None,
            cache_key: "cache".into(),
        },
    )]);
    let timeline = scene_audio_timeline(&script, &root.0, &prepared, &origins).unwrap();
    assert!(matches!(
        &timeline.clips[1].source,
        AudioSourceInfo::Synthesized { provider, voice, .. } if provider == "kokoro" && voice == "af_heart"
    ));
}
