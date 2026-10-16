//! Blocking preparation for one complete scene; callers must run this off the UI thread.
use crate::{dialogue_jobs, export, timeline::DialogueClip, DialogueJob};
use motion_lipsync::cache::{AlignmentCache, AlignmentInput, AlignmentStatus};
use motion_lipsync::Alignment;
use motion_script::Script;
use motion_tts::{CancellationToken, SpeechError, SpeechProvider, SpeechResult};
use std::{collections::BTreeMap, path::Path};

pub struct PreparedScene {
    pub scene_id: String,
    pub rendered: export::RenderedScene,
    pub clips: Vec<DialogueClip>,
}

pub struct PreparedAlignedScene {
    pub prepared: PreparedScene,
    pub alignment_statuses: BTreeMap<u64, AlignmentStatus>,
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
    progress: impl FnMut(SceneProgress),
) -> Result<PreparedScene, SpeechError> {
    prepare_scene_with_cancellation(
        script,
        scene_index,
        project_root,
        provider,
        alignments,
        &CancellationToken::default(),
        progress,
    )
}

/// Cancellation is checked at stage boundaries; partial scenes are never returned.
#[allow(clippy::too_many_arguments)]
pub fn prepare_scene_with_cancellation(
    script: &Script,
    scene_index: usize,
    project_root: &Path,
    provider: &dyn SpeechProvider,
    alignments: &BTreeMap<u64, Alignment>,
    cancel: &CancellationToken,
    progress: impl FnMut(SceneProgress),
) -> Result<PreparedScene, SpeechError> {
    prepare_scene_common(
        script,
        scene_index,
        project_root,
        provider,
        alignments.keys().copied(),
        cancel,
        progress,
        |job, result| DialogueClip::prepare(job, result, alignments.get(&job.id).cloned()),
    )
}

/// Explicit bound inputs and cache lookups; no alignment inference is performed.
#[allow(clippy::too_many_arguments)]
pub fn prepare_scene_with_alignment_inputs(
    script: &Script,
    scene_index: usize,
    project_root: &Path,
    provider: &dyn SpeechProvider,
    inputs: &BTreeMap<u64, AlignmentInput>,
    cache: &AlignmentCache,
    cancel: &CancellationToken,
    progress: impl FnMut(SceneProgress),
) -> Result<PreparedAlignedScene, SpeechError> {
    let mut statuses = BTreeMap::new();
    let prepared = prepare_scene_common(
        script,
        scene_index,
        project_root,
        provider,
        inputs.keys().copied(),
        cancel,
        progress,
        |job, result| {
            let (clip, status) = DialogueClip::prepare_with_alignment_input(
                job,
                result,
                inputs.get(&job.id),
                cache,
            )?;
            statuses.insert(job.id, status);
            Ok(clip)
        },
    )?;
    Ok(PreparedAlignedScene {
        prepared,
        alignment_statuses: statuses,
    })
}

#[allow(clippy::too_many_arguments)]
fn prepare_scene_common(
    script: &Script,
    scene_index: usize,
    project_root: &Path,
    provider: &dyn SpeechProvider,
    mut alignment_ids: impl Iterator<Item = u64>,
    cancel: &CancellationToken,
    mut progress: impl FnMut(SceneProgress),
    mut prepare: impl FnMut(&DialogueJob, &SpeechResult) -> Result<DialogueClip, SpeechError>,
) -> Result<PreparedScene, SpeechError> {
    cancel.check()?;
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
    if let Some(id) = alignment_ids.find(|id| !jobs.iter().any(|job| job.id == *id)) {
        return Err(SpeechError(format!(
            "alignment dialogue ID {id} does not belong to scene {}",
            scene.id
        )));
    }
    let total = jobs.len();
    let mut clips = Vec::with_capacity(total);
    for job in jobs {
        cancel.check()?;
        let context = |error: SpeechError| {
            SpeechError(format!(
                "scene {} dialogue {} ({}): {}",
                scene.id, job.id, job.actor, error
            ))
        };
        let result = match &job.imported_audio {
            Some(path) => super::imported(path),
            None => std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                provider.synthesize_cancellable(job.request.clone(), cancel)
            }))
            .unwrap_or_else(|_| Err(SpeechError("speech provider panicked; job failed".into()))),
        }
        .map_err(context)?;
        cancel.check()?;
        let clip = prepare(&job, &result).map_err(context)?;
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
    cancel.check()?;
    let rendered = export::render_scene(&scene.id, scene.duration, &clips)?;
    cancel.check()?;
    Ok(PreparedScene {
        scene_id: scene.id.clone(),
        rendered,
        clips,
    })
}

#[cfg(test)]
mod bound_tests {
    use super::*;
    use motion_audio::{write_wav, AudioBuffer};
    use motion_lipsync::{
        cache::{BoundAlignment, CacheKey},
        AlignmentSource, TimedPhoneme,
    };
    use motion_tts::SpeechRequest;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicUsize, Ordering},
    };
    struct Fixture {
        root: PathBuf,
        calls: AtomicUsize,
    }
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);
            let root = std::env::temp_dir().join(format!(
                "bound-scene-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            std::fs::create_dir_all(&root).unwrap();
            write_wav(
                root.join("voice.wav"),
                &AudioBuffer {
                    sample_rate: 24000,
                    channels: 1,
                    samples: vec![0.25; 24],
                },
            )
            .unwrap();
            Self {
                root,
                calls: AtomicUsize::new(0),
            }
        }
        fn input(&self) -> AlignmentInput {
            let audio = motion_audio::read_wav(self.root.join("voice.wav")).unwrap();
            let key = CacheKey::new(&audio, "Hello", "manual", "1", "", "edit-1").unwrap();
            AlignmentInput::Bound(
                BoundAlignment::new(
                    key,
                    Some(Alignment {
                        source: AlignmentSource::Manual,
                        duration_seconds: 0.001,
                        phonemes: vec![TimedPhoneme {
                            phoneme: "m".into(),
                            start_seconds: 0.0,
                            end_seconds: 0.001,
                        }],
                    }),
                )
                .unwrap(),
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }
    impl SpeechProvider for Fixture {
        fn synthesize(&self, _: SpeechRequest) -> Result<SpeechResult, SpeechError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(SpeechResult {
                audio_path: self.root.join("voice.wav"),
                sample_rate: 24000,
                sample_count: 24,
                cache_key: String::new(),
                phonemes: String::new(),
                cached: false,
            })
        }
    }
    fn script() -> Script {
        motion_script::parse("characters:\n  EMMA: {asset: characters/emma, voice: {provider: kokoro, preset: af_heart, language: en-us, speed: 1.0}}\nscenes:\n  - id: scene\n    duration: 0.01\n    events:\n      - {type: speak, at: 0.0, actor: EMMA, text: Hello}\n").unwrap()
    }
    #[test]
    fn exact_binding_single_scene_cache_reuse_and_corruption() {
        let fixture = Fixture::new();
        let script = script();
        let job = dialogue_jobs(&script, &fixture.root).unwrap().remove(0);
        let result = fixture.synthesize(job.request.clone()).unwrap();
        let cache = AlignmentCache::new(fixture.root.join("cache"));
        let input = fixture.input();
        let (single, status) =
            DialogueClip::prepare_with_alignment_input(&job, &result, Some(&input), &cache)
                .unwrap();
        assert_eq!(status, AlignmentStatus::BoundImported);
        let scene = prepare_scene_with_alignment_inputs(
            &script,
            0,
            &fixture.root,
            &fixture,
            &BTreeMap::from([(0, input.clone())]),
            &cache,
            &CancellationToken::default(),
            |_| {},
        )
        .unwrap();
        assert_eq!(scene.alignment_statuses[&0], AlignmentStatus::BoundCacheHit);
        assert_eq!(
            single.mouth_at(0).unwrap(),
            scene.prepared.clips[0].mouth_at(0).unwrap()
        );
        let file = std::fs::read_dir(fixture.root.join("cache"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path();
        std::fs::write(file, b"corrupt").unwrap();
        assert!(prepare_scene_with_alignment_inputs(
            &script,
            0,
            &fixture.root,
            &fixture,
            &BTreeMap::from([(0, input)]),
            &cache,
            &CancellationToken::default(),
            |_| panic!("no progress on corrupt input")
        )
        .is_err());
    }
    #[test]
    fn same_duration_wrong_pcm_and_changed_current_text_are_rejected() {
        let fixture = Fixture::new();
        let job = dialogue_jobs(&script(), &fixture.root).unwrap().remove(0);
        let input = fixture.input();
        let result = fixture.synthesize(job.request.clone()).unwrap();
        let cache = AlignmentCache::new(fixture.root.join("cache"));
        let mut changed_job = job.clone();
        changed_job.request.text = "Changed".into();
        assert!(DialogueClip::prepare_with_alignment_input(
            &changed_job,
            &result,
            Some(&input),
            &cache
        )
        .is_err());
        write_wav(
            &result.audio_path,
            &AudioBuffer {
                sample_rate: 24000,
                channels: 1,
                samples: vec![-0.25; 24],
            },
        )
        .unwrap();
        assert!(
            DialogueClip::prepare_with_alignment_input(&job, &result, Some(&input), &cache)
                .is_err()
        );
        assert!(!fixture.root.join("cache").exists());
    }
    #[test]
    fn bound_scene_cancel_and_foreign_ids_do_not_synthesize() {
        let fixture = Fixture::new();
        let cache = AlignmentCache::new(fixture.root.join("cache"));
        let cancel = CancellationToken::default();
        cancel.cancel();
        assert!(prepare_scene_with_alignment_inputs(
            &script(),
            0,
            &fixture.root,
            &fixture,
            &BTreeMap::from([(0, fixture.input())]),
            &cache,
            &cancel,
            |_| panic!("cancelled")
        )
        .is_err());
        assert!(prepare_scene_with_alignment_inputs(
            &script(),
            0,
            &fixture.root,
            &fixture,
            &BTreeMap::from([(99, fixture.input())]),
            &cache,
            &CancellationToken::default(),
            |_| panic!("foreign ID")
        )
        .is_err());
        assert_eq!(fixture.calls.load(Ordering::SeqCst), 0);
        assert!(!fixture.root.join("cache").exists());
    }
}
