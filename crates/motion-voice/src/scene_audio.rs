//! Blocking preparation for one complete scene; callers must run this off the UI thread.
use crate::{dialogue_jobs, export, timeline::DialogueClip};
use motion_lipsync::Alignment;
use motion_script::Script;
use motion_tts::{SpeechError, SpeechProvider};
use std::{collections::BTreeMap, path::Path};

pub struct PreparedScene {
    pub scene_id: String,
    pub rendered: export::RenderedScene,
    pub clips: Vec<DialogueClip>,
}

#[derive(Clone, Debug)]
pub struct SceneProgress {
    pub completed: usize,
    pub total: usize,
    pub dialogue_id: u64,
    pub actor: String,
}

/// Synthesizes or imports each selected dialogue and mixes on the export sample clock.
/// Alignment IDs are the stable global IDs returned by `dialogue_jobs`, not scene-local
/// indices. Missing alignments leave mouths at rest; timestamps are never invented.
/// Failure returns no partially prepared scene, and progress counts prepared clips only.
pub fn prepare_scene(
    script: &Script,
    scene_index: usize,
    project_root: &Path,
    provider: &dyn SpeechProvider,
    alignments: &BTreeMap<u64, Alignment>,
    mut progress: impl FnMut(SceneProgress),
) -> Result<PreparedScene, SpeechError> {
    let scene = script
        .scenes
        .get(scene_index)
        .ok_or_else(|| SpeechError(format!("scene index {scene_index} does not exist")))?;
    if !scene.duration.is_finite() || !(0.0..=export::MAX_SCENE_SECONDS).contains(&scene.duration) {
        return Err(SpeechError(
            "scene audio duration must be finite and within one hour".into(),
        ));
    }
    let jobs: Vec<_> = dialogue_jobs(script, project_root)?
        .into_iter()
        .filter(|job| job.scene == scene.id)
        .collect();
    if let Some(id) = alignments
        .keys()
        .find(|id| !jobs.iter().any(|job| job.id == **id))
    {
        return Err(SpeechError(format!(
            "alignment dialogue ID {id} does not belong to scene {}",
            scene.id
        )));
    }
    let total = jobs.len();
    let mut clips = Vec::with_capacity(total);
    for job in jobs {
        let context = |error: SpeechError| {
            SpeechError(format!(
                "scene {} dialogue {} ({}): {}",
                scene.id, job.id, job.actor, error
            ))
        };
        let result = match &job.imported_audio {
            Some(path) => super::imported(path),
            None => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                provider.synthesize(job.request.clone())
            }))
            .unwrap_or_else(|_| Err(SpeechError("speech provider panicked; job failed".into()))),
        }
        .map_err(context)?;
        let clip = DialogueClip::prepare(&job, &result, alignments.get(&job.id).cloned())
            .map_err(context)?;
        if clips.iter().any(|existing: &DialogueClip| {
            existing.actor == clip.actor
                && existing.start_frame < clip.start_frame + clip.audio.frames() as u64
                && clip.start_frame < existing.start_frame + existing.audio.frames() as u64
        }) {
            return Err(SpeechError(format!(
                "scene {} dialogue {} ({}): overlapping dialogue for the same actor; adjust timing",
                scene.id, job.id, job.actor
            )));
        }
        clips.push(clip);
        progress(SceneProgress {
            completed: clips.len(),
            total,
            dialogue_id: job.id,
            actor: job.actor,
        });
    }
    let rendered = export::render_scene(&scene.id, scene.duration, &clips)?;
    Ok(PreparedScene {
        scene_id: scene.id.clone(),
        rendered,
        clips,
    })
}
