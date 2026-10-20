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
    pub alignments: String,
    pub cancel: CancellationToken,
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
                    let prepared = prepare_scene_with_alignment_inputs(
                        &request.script,
                        request.index,
                        &request.root,
                        &motion_tts::KokoroProvider {
                            config: request.config,
                        },
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
                    Err("Scene preparation worker panicked; retry after checking inputs".into())
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
            .map_err(|e| format!("Scene preparation queue: {e}"))
    }
    pub fn poll(&self) -> Option<Update> {
        self.rx.try_recv().ok()
    }
}

const MAX_ALIGNMENT_JSON_BYTES: usize = 100_000;
fn check_json_size(input: &str) -> Result<(), String> {
    if input.len() > MAX_ALIGNMENT_JSON_BYTES {
        return Err("Alignment JSON exceeds 100 KB; shorten the input and retry".into());
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
        .map_err(|e| format!("Alignment JSON: {e}"))
}
fn parse_alignment_inputs(input: &str) -> Result<BTreeMap<u64, AlignmentInput>, String> {
    check_json_size(input)?;
    if input.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    serde_json::from_str(input).map_err(|e| format!("Scene alignment JSON: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
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
