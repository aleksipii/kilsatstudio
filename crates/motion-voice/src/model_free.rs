//! Strict model-free dialogue preparation (ADR-A01-03, ADR-A01-09).
//!
//! The policy decision (`motion_project::ModelPolicy::permit`) belongs to the caller;
//! this module only provides the fail-closed preparation for when it says `Strict`:
//! every line must name an existing imported WAV, which is checked before any
//! preparation starts, and the provider is `DisabledSpeechProvider`, so no synthesis
//! can run even if a check were missed. There is no automatic fallback.
use crate::{dialogue_jobs, scene_audio, DialogueJob};
use motion_lipsync::Alignment;
use motion_script::Script;
use motion_tts::{CancellationToken, DisabledSpeechProvider, SpeechError};
use std::{collections::BTreeMap, fmt, path::Path, path::PathBuf};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MissingAudioReason {
    /// The `speak` event has no `audio:` field.
    NotSpecified,
    /// The named file does not exist or is not a regular file.
    FileMissing(PathBuf),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissingDialogueAudio {
    pub dialogue_id: u64,
    pub scene: String,
    pub actor: String,
    pub reason: MissingAudioReason,
}

#[derive(Debug)]
pub enum ModelFreeError {
    /// Lines that would need synthesis. Nothing was prepared.
    MissingImportedAudio(Vec<MissingDialogueAudio>),
    /// Preparation of the imported audio itself failed.
    Preparation(SpeechError),
}
impl fmt::Display for ModelFreeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingImportedAudio(lines) => {
                write!(
                    f,
                    "strict model-free mode needs an imported WAV for every line; missing:"
                )?;
                for line in lines {
                    match &line.reason {
                        MissingAudioReason::NotSpecified => write!(
                            f,
                            " scene {} dialogue {} ({}) has no audio;",
                            line.scene, line.dialogue_id, line.actor
                        )?,
                        MissingAudioReason::FileMissing(path) => write!(
                            f,
                            " scene {} dialogue {} ({}) file {} not found;",
                            line.scene,
                            line.dialogue_id,
                            line.actor,
                            path.display()
                        )?,
                    }
                }
                Ok(())
            }
            Self::Preparation(error) => write!(f, "{error}"),
        }
    }
}
impl std::error::Error for ModelFreeError {}

/// Checks, without reading audio or calling a provider, that every dialogue line of
/// the scene (or of all scenes when `scene_index` is `None`) names an existing WAV.
pub fn require_imported_audio(
    script: &Script,
    scene_index: Option<usize>,
    project_root: &Path,
) -> Result<Vec<DialogueJob>, ModelFreeError> {
    let scene_id = match scene_index {
        Some(index) => Some(
            script
                .scenes
                .get(index)
                .ok_or_else(|| {
                    ModelFreeError::Preparation(SpeechError(format!(
                        "scene index {index} does not exist"
                    )))
                })?
                .id
                .clone(),
        ),
        None => None,
    };
    let jobs: Vec<DialogueJob> = dialogue_jobs(script, project_root)
        .map_err(ModelFreeError::Preparation)?
        .into_iter()
        .filter(|job| scene_id.as_ref().is_none_or(|id| &job.scene == id))
        .collect();
    let missing: Vec<_> = jobs
        .iter()
        .filter_map(|job| {
            let reason = match &job.imported_audio {
                None => MissingAudioReason::NotSpecified,
                Some(path) if !path.is_file() => MissingAudioReason::FileMissing(path.clone()),
                Some(_) => return None,
            };
            Some(MissingDialogueAudio {
                dialogue_id: job.id,
                scene: job.scene.clone(),
                actor: job.actor.clone(),
                reason,
            })
        })
        .collect();
    if missing.is_empty() {
        Ok(jobs)
    } else {
        Err(ModelFreeError::MissingImportedAudio(missing))
    }
}

/// `prepare_scene_with_cancellation` on the strict path: checks imported audio first,
/// then prepares with `DisabledSpeechProvider`. Mouth timing comes only from the given
/// alignments (manual/imported) or the rule-based text estimate.
pub fn prepare_scene_model_free(
    script: &Script,
    scene_index: usize,
    project_root: &Path,
    alignments: &BTreeMap<u64, Alignment>,
    cancel: &CancellationToken,
    progress: impl FnMut(scene_audio::SceneProgress),
) -> Result<scene_audio::PreparedScene, ModelFreeError> {
    require_imported_audio(script, Some(scene_index), project_root)?;
    scene_audio::prepare_scene_with_cancellation(
        script,
        scene_index,
        project_root,
        &DisabledSpeechProvider,
        alignments,
        cancel,
        progress,
    )
    .map_err(ModelFreeError::Preparation)
}
