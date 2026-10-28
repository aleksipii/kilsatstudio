//! One persistent, bounded worker for complete scene preparation.
use motion_lipsync::cache::{AlignmentCache, AlignmentInput, AlignmentStatus};
use motion_script::Script;
use motion_tts::{CancellationToken, KokoroConfig};
use motion_voice::scene_audio::{
    prepare_scene_with_alignment_inputs, PreparedScene, SceneProgress,
};
use std::{collections::BTreeMap, path::PathBuf, sync::mpsc};

pub struct Request {
    pub script: Script,
    pub index: usize,
    pub root: PathBuf,
    pub config: KokoroConfig,
    pub piper_config: Option<PathBuf>,
    pub alignments: String,
    pub cancel: CancellationToken,
    /// Project model policy. `Strict` checks imported WAVs first and never builds
    /// a speech provider (ADR-A01-09).
    pub policy: motion_project::ModelPolicy,
}
pub struct Ready {
    pub prepared: PreparedScene,
    pub alignment_statuses: BTreeMap<u64, AlignmentStatus>,
    pub peaks: Vec<motion_audio::WaveformPeak>,
}
#[allow(clippy::large_enum_variant)] // short-lived channel messages
pub enum Update {
    Progress(SceneProgress),
    Finished(Result<Ready, String>),
}
pub struct Worker {
    tx: mpsc::SyncSender<Request>,
    rx: mpsc::Receiver<Update>,
}
impl Worker {
    pub fn new() -> Self {
        let (tx, requests) = mpsc::sync_channel::<Request>(1);
        let (updates, rx) = mpsc::sync_channel(4);
        std::thread::spawn(move || {
            while let Ok(request) = requests.recv() {
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    request.cancel.check().map_err(|e| e.to_string())?;
                    let alignments = parse_alignment_inputs(&request.alignments)?;
                    let cache =
                        AlignmentCache::new(request.config.cache_directory.join("alignments"));
                    let provider = speech_provider(
                        request.policy,
                        &request.script,
                        Some(request.index),
                        &request.root,
                        &request.config,
                        request.piper_config.as_ref(),
                    )?;
                    let prepared = prepare_scene_with_alignment_inputs(
                        &request.script,
                        request.index,
                        &request.root,
                        provider.as_ref(),
                        &alignments,
                        &cache,
                        &request.cancel,
                        |progress| {
                            let _ = updates.try_send(Update::Progress(progress));
                        },
                    )
                    .map_err(|e| e.to_string())?;
                    request.cancel.check().map_err(|e| e.to_string())?;
                    let alignment_statuses = prepared.alignment_statuses;
                    let prepared = prepared.prepared;
                    let audio = &prepared.rendered.audio;
                    let peaks = motion_audio::waveform_peaks(audio, (audio.frames() / 700).max(1))
                        .map_err(|e| e.to_string())?;
                    request.cancel.check().map_err(|e| e.to_string())?;
                    Ok(Ready {
                        prepared,
                        peaks,
                        alignment_statuses,
                    })
                }))
                .unwrap_or_else(|_| {
                    Err(
                        "Kohtauksen valmistelija kaatui; tarkista syötteet ja yritä uudelleen"
                            .into(),
                    )
                });
                if updates.send(Update::Finished(result)).is_err() {
                    break;
                }
            }
        });
        Self { tx, rx }
    }
    pub fn submit(&self, request: Request) -> Result<(), String> {
        self.tx
            .try_send(request)
            .map_err(|e| format!("Kohtauksen valmistelujono: {e}"))
    }
    pub fn poll(&self) -> Option<Update> {
        self.rx.try_recv().ok()
    }
}

/// The provider for one preparation. On the strict path every line must already name
/// an existing imported WAV (checked before any work) and the provider is
/// `DisabledSpeechProvider`, so synthesis cannot run even if a check were missed.
pub fn speech_provider(
    policy: motion_project::ModelPolicy,
    script: &Script,
    scene_index: Option<usize>,
    root: &std::path::Path,
    config: &KokoroConfig,
    piper_config: Option<&PathBuf>,
) -> Result<Box<dyn motion_tts::SpeechProvider>, String> {
    if policy.is_strict() {
        motion_voice::model_free::require_imported_audio(script, scene_index, root).map_err(
            |e| format!("No AI -tila: jokaisella repliikillä on oltava tuotu WAV-tiedosto ({e})"),
        )?;
        return Ok(Box::new(motion_tts::DisabledSpeechProvider));
    }
    let mut provider = motion_tts::LocalSpeechProvider::new(config.clone());
    if let Some(path) = piper_config {
        provider = provider.with_piper_config_path(path.clone());
    }
    Ok(Box::new(provider))
}

const MAX_ALIGNMENT_JSON_BYTES: usize = 100_000;
fn check_json_size(input: &str) -> Result<(), String> {
    if input.len() > MAX_ALIGNMENT_JSON_BYTES {
        return Err(
            "Puheen kohdistuksen JSON ylittää 100 KB:n rajan; lyhennä syötettä ja yritä uudelleen"
                .into(),
        );
    }
    Ok(())
}
pub fn parse_alignment_input(input: &str) -> Result<Option<AlignmentInput>, String> {
    check_json_size(input)?;
    if input.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str(input)
        .map(Some)
        .map_err(|e| format!("Puheen kohdistuksen JSON: {e}"))
}
fn parse_alignment_inputs(input: &str) -> Result<BTreeMap<u64, AlignmentInput>, String> {
    check_json_size(input)?;
    if input.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    serde_json::from_str(input).map_err(|e| format!("Kohtauksen puhekohdistuksen JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_policy_requires_imported_wavs_and_never_builds_a_synthesizer() {
        use motion_project::ModelPolicy;
        use motion_tts::{SpeechRequest, VoiceSettings};
        let root = std::env::temp_dir().join(format!("kilsat-a09-strict-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let script = |audio: &str| {
            motion_script::parse(&format!(
                "characters: {{EMMA: {{asset: characters/emma, voice: {{provider: kokoro, preset: af_heart, language: en-us, speed: 1.0}}}}}}\nscenes:\n  - {{id: a, duration: 2, events: [{{type: speak, at: 0.0, actor: EMMA, text: Hi{audio}}}]}}\n"
            ))
            .unwrap()
        };
        let config = KokoroConfig {
            python: "/never-execute/python".into(),
            runtime_script: "/never-execute/runtime".into(),
            model_config: "/never-execute/config".into(),
            model_weights: "/never-execute/model".into(),
            voices_directory: "/never-execute/voices".into(),
            cache_directory: root.join("cache"),
            model_version: "test".into(),
            timeout: std::time::Duration::from_secs(1),
        };
        let missing = speech_provider(
            ModelPolicy::Strict,
            &script(""),
            Some(0),
            &root,
            &config,
            None,
        );
        let error = missing
            .err()
            .expect("strict path must reject a line without WAV");
        assert!(error.contains("No AI"), "{error}");
        std::fs::write(root.join("line.wav"), b"placeholder").unwrap();
        let provider = speech_provider(
            ModelPolicy::Strict,
            &script(", audio: line.wav"),
            Some(0),
            &root,
            &config,
            None,
        )
        .unwrap();
        let request = SpeechRequest {
            text: "Hi".into(),
            voice: VoiceSettings::default(),
        };
        let error = provider.synthesize(request).unwrap_err();
        assert_eq!(error.0, motion_tts::DisabledSpeechProvider::MESSAGE);
        // The default policy keeps today's local provider.
        assert!(speech_provider(
            ModelPolicy::default(),
            &script(""),
            Some(0),
            &root,
            &config,
            None
        )
        .is_ok());
        let _ = std::fs::remove_dir_all(&root);
    }
    #[test]
    fn blank_alignment_never_invents_timing() {
        assert!(parse_alignment_input(" \n ").unwrap().is_none());
        assert!(parse_alignment_inputs("").unwrap().is_empty());
    }
    #[test]
    fn legacy_alignment_is_explicitly_unbound() {
        let json = serde_json::to_string(&motion_lipsync::Alignment {
            source: motion_lipsync::AlignmentSource::Manual,
            duration_seconds: 1.0,
            phonemes: vec![],
        })
        .unwrap();
        assert!(matches!(
            parse_alignment_input(&json).unwrap(),
            Some(AlignmentInput::Legacy(_))
        ));
        let map = format!("{{\"42\":{json}}}");
        assert!(matches!(
            parse_alignment_inputs(&map).unwrap().get(&42),
            Some(AlignmentInput::Legacy(_))
        ));
    }
    #[test]
    fn bound_cache_only_envelope_is_parsed_without_losing_identity() {
        use motion_lipsync::cache::{BoundAlignment, CacheKey};
        let audio = motion_audio::AudioBuffer {
            sample_rate: 24000,
            channels: 1,
            samples: vec![0.0; 24],
        };
        let key = CacheKey::new(&audio, "hello", "manual", "1", "test", "edit-1").unwrap();
        let json = serde_json::to_string(&BoundAlignment::new(key, None).unwrap()).unwrap();
        let Some(AlignmentInput::Bound(bound)) = parse_alignment_input(&json).unwrap() else {
            panic!("bound envelope was not preserved");
        };
        assert!(bound.alignment.is_none());
        assert_eq!(bound.schema_version, 1);
        assert_eq!(bound.key.audio_sha256().len(), 64);
    }
    #[test]
    fn oversized_and_malformed_json_are_recoverable() {
        assert!(parse_alignment_input(&" ".repeat(MAX_ALIGNMENT_JSON_BYTES + 1)).is_err());
        assert!(parse_alignment_inputs("not JSON").is_err());
        assert!(parse_alignment_inputs("{}").is_ok());
    }
}
