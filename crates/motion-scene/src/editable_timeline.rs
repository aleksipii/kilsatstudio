//! Versioned scene timing edits, compiled explicitly through the shared sequence clock.
//!
//! This document stores scene order and durations, not a second evaluated timeline.
//! Applying it preserves source events and composition; invalid trims or transitions
//! are rejected by the ordinary compiler. Event retiming is deliberately explicit
//! future work rather than silently scaling speech or animation keys.
use crate::SequenceTimeline;
use motion_script::Script;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const TIMELINE_EDIT_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneTiming {
    pub scene_id: String,
    pub duration: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditableTimeline {
    pub version: u32,
    pub scenes: Vec<SceneTiming>,
}

/// Commands refer to stable scene IDs, never stale display row indices.
#[derive(Debug, Clone, PartialEq)]
pub enum TimelineEdit {
    SetDuration {
        scene_id: String,
        duration: f64,
    },
    /// Move before another scene, or to the end when `before` is `None`.
    MoveScene {
        scene_id: String,
        before: Option<String>,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum TimelineEditError {
    #[error("unsupported editable timeline version {0}")]
    Version(u32),
    #[error("editable timeline requires every source scene exactly once (1..1024 scenes)")]
    SceneSet,
    #[error("unknown scene reference: {0}")]
    UnknownScene(String),
    #[error("scene {0}: duration must be finite and positive")]
    Duration(String),
    #[error("edited timeline failed shared sequence validation: {0}")]
    Compile(#[from] crate::Error),
}

/// The validated source and timeline must travel together: contribution indices
/// refer to this source's reordered scenes, not the original script's indices.
#[derive(Debug, Clone)]
pub struct CompiledTimelineEdit {
    pub script: Script,
    pub timeline: SequenceTimeline,
}

impl EditableTimeline {
    pub fn from_script(script: &Script) -> Result<Self, TimelineEditError> {
        SequenceTimeline::compile(script)?;
        Ok(Self {
            version: TIMELINE_EDIT_VERSION,
            scenes: script
                .scenes
                .iter()
                .map(|scene| SceneTiming {
                    scene_id: scene.id.clone(),
                    duration: scene.duration,
                })
                .collect(),
        })
    }

    /// Rebuild using the existing compiler and its single 24 kHz sample clock.
    /// A deserialized document is untrusted until this succeeds.
    pub fn compile(&self, source: &Script) -> Result<CompiledTimelineEdit, TimelineEditError> {
        if self.version != TIMELINE_EDIT_VERSION {
            return Err(TimelineEditError::Version(self.version));
        }
        if self.scenes.is_empty()
            || self.scenes.len() > 1024
            || self.scenes.len() != source.scenes.len()
        {
            return Err(TimelineEditError::SceneSet);
        }
        // Validate the original too: edits must not repair malformed source by
        // accidentally discarding duplicate IDs or invalid original durations.
        SequenceTimeline::compile(source)?;
        let by_id: BTreeMap<_, _> = source
            .scenes
            .iter()
            .map(|scene| (scene.id.as_str(), scene))
            .collect();
        let mut seen = BTreeSet::new();
        let mut script = source.clone();
        script.scenes.clear();
        for timing in &self.scenes {
            if !seen.insert(timing.scene_id.as_str()) {
                return Err(TimelineEditError::SceneSet);
            }
            if !timing.duration.is_finite() || timing.duration <= 0.0 {
                return Err(TimelineEditError::Duration(timing.scene_id.clone()));
            }
            let mut scene = (*by_id
                .get(timing.scene_id.as_str())
                .ok_or_else(|| TimelineEditError::UnknownScene(timing.scene_id.clone()))?)
            .clone();
            scene.duration = timing.duration;
            script.scenes.push(scene);
        }
        let timeline = SequenceTimeline::compile(&script)?;
        Ok(CompiledTimelineEdit { script, timeline })
    }

    /// Commit a command only after the entire candidate compiles. On failure,
    /// both the edit document and caller-owned source remain untouched.
    pub fn apply(
        &mut self,
        source: &Script,
        edit: TimelineEdit,
    ) -> Result<CompiledTimelineEdit, TimelineEditError> {
        self.compile(source)?;
        let mut candidate = self.clone();
        match edit {
            TimelineEdit::SetDuration { scene_id, duration } => {
                let scene = candidate
                    .scenes
                    .iter_mut()
                    .find(|scene| scene.scene_id == scene_id)
                    .ok_or_else(|| TimelineEditError::UnknownScene(scene_id.clone()))?;
                scene.duration = duration;
            }
            TimelineEdit::MoveScene { scene_id, before } => {
                let from = candidate
                    .scenes
                    .iter()
                    .position(|scene| scene.scene_id == scene_id)
                    .ok_or_else(|| TimelineEditError::UnknownScene(scene_id.clone()))?;
                if before.as_ref() != Some(&scene_id) {
                    if let Some(target) = &before {
                        if !candidate
                            .scenes
                            .iter()
                            .any(|scene| &scene.scene_id == target)
                        {
                            return Err(TimelineEditError::UnknownScene(target.clone()));
                        }
                    }
                    let scene = candidate.scenes.remove(from);
                    let to = before
                        .as_ref()
                        .and_then(|target| {
                            candidate
                                .scenes
                                .iter()
                                .position(|scene| &scene.scene_id == target)
                        })
                        .unwrap_or(candidate.scenes.len());
                    candidate.scenes.insert(to, scene);
                }
            }
        }
        let compiled = candidate.compile(source)?;
        *self = candidate;
        Ok(compiled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motion_script::Transition;

    fn source() -> Script {
        motion_script::parse("characters: {}\nscenes: [{id: a, duration: 2, events: []}, {id: b, duration: 1, events: []}]").unwrap()
    }

    #[test]
    fn roundtrip_reorders_stable_ids_and_rebuilds_shared_clock() {
        let source = source();
        let mut document = EditableTimeline::from_script(&source).unwrap();
        document
            .apply(
                &source,
                TimelineEdit::MoveScene {
                    scene_id: "b".into(),
                    before: Some("a".into()),
                },
            )
            .unwrap();
        let compiled = document
            .apply(
                &source,
                TimelineEdit::SetDuration {
                    scene_id: "b".into(),
                    duration: 0.5,
                },
            )
            .unwrap();
        let json = serde_json::to_string(&document).unwrap();
        let restored: EditableTimeline = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, document);
        let rebuilt = restored.compile(&source).unwrap();
        assert_eq!(rebuilt.timeline.spans(), compiled.timeline.spans());
        assert_eq!(compiled.timeline.total_samples(), 60_000);
        assert_eq!(compiled.script.scenes[0].id, "b");
        assert_eq!(compiled.timeline.sample(12_000).unwrap()[0].scene_index, 1);
        assert_eq!(source.scenes[0].id, "a");
        assert_eq!(source.scenes[1].duration, 1.0);
    }

    #[test]
    fn failed_retimes_and_references_are_atomic() {
        let source = source();
        let mut document = EditableTimeline::from_script(&source).unwrap();
        let before = document.clone();
        for duration in [f64::NAN, f64::INFINITY, -1.0, 0.0, 0.000001, f64::MAX] {
            assert!(document
                .apply(
                    &source,
                    TimelineEdit::SetDuration {
                        scene_id: "a".into(),
                        duration,
                    }
                )
                .is_err());
            assert_eq!(document, before);
        }
        assert!(document
            .apply(
                &source,
                TimelineEdit::MoveScene {
                    scene_id: "a".into(),
                    before: Some("missing".into()),
                }
            )
            .is_err());
        assert_eq!(document, before);
        assert!(document
            .apply(
                &source,
                TimelineEdit::SetDuration {
                    scene_id: "missing".into(),
                    duration: 1.0,
                }
            )
            .is_err());
        assert_eq!(document, before);
    }

    #[test]
    fn untrusted_versions_duplicates_and_missing_scenes_are_rejected() {
        let source = source();
        let original = EditableTimeline::from_script(&source).unwrap();
        let mut bad = original.clone();
        bad.version = 2;
        assert!(matches!(
            bad.compile(&source),
            Err(TimelineEditError::Version(2))
        ));
        bad = original.clone();
        bad.scenes[1].scene_id = "a".into();
        assert!(matches!(
            bad.compile(&source),
            Err(TimelineEditError::SceneSet)
        ));
        bad = original.clone();
        bad.scenes.pop();
        assert!(bad.compile(&source).is_err());
        bad = original;
        bad.scenes[0].scene_id = "unknown".into();
        assert!(matches!(
            bad.compile(&source),
            Err(TimelineEditError::UnknownScene(_))
        ));
        assert!(serde_json::from_str::<EditableTimeline>(
            r#"{"version":1,"scenes":[],"ignored":true}"#
        )
        .is_err());
    }

    #[test]
    fn retiming_preserves_authored_tracks_and_rejects_truncation() {
        let mut source = source();
        source.scenes[0]
            .composition
            .camera
            .push(motion_script::CameraKey {
                at: 1.5,
                x: 4.0,
                y: 8.0,
                zoom: 1.25,
            });
        let mut document = EditableTimeline::from_script(&source).unwrap();
        let before = document.clone();
        assert!(document
            .apply(
                &source,
                TimelineEdit::SetDuration {
                    scene_id: "a".into(),
                    duration: 1.0,
                }
            )
            .is_err());
        assert_eq!(document, before);
        let compiled = document
            .apply(
                &source,
                TimelineEdit::SetDuration {
                    scene_id: "a".into(),
                    duration: 3.0,
                },
            )
            .unwrap();
        assert_eq!(
            compiled.script.scenes[0].composition,
            source.scenes[0].composition
        );
        assert_eq!(source.scenes[0].duration, 2.0);
    }

    #[test]
    fn transitions_use_existing_validation_and_random_seek_weights() {
        let mut source = source();
        source.scenes[0].composition.transition = Transition::Crossfade { duration: 0.5 };
        let mut document = EditableTimeline::from_script(&source).unwrap();
        let before = document.clone();
        // Moving an outgoing crossfade to the last scene is invalid, not silently cut.
        assert!(document
            .apply(
                &source,
                TimelineEdit::MoveScene {
                    scene_id: "a".into(),
                    before: None,
                }
            )
            .is_err());
        assert_eq!(document, before);
        let compiled = document.compile(&source).unwrap();
        for sample in [42_000, 36_000, 47_999, 40_000] {
            let contributions = compiled.timeline.sample(sample).unwrap();
            assert_eq!(contributions.len(), 2);
            assert_eq!(
                contributions.iter().map(|value| value.weight).sum::<f64>(),
                1.0
            );
        }
    }
}
