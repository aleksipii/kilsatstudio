//! Deterministic scene audio and mouth-track export on the 24 kHz sample clock.
//! This is an audio/metadata interchange, not a video encoder or alignment certification.
use crate::timeline::DialogueClip;
use motion_audio::AudioBuffer;
use motion_lipsync::{AlignmentSource, Viseme};
use motion_tts::SpeechError;
use serde::{Deserialize, Serialize};

pub const SAMPLE_RATE: u32 = 24_000;
const MAX_SCENE_SECONDS: f64 = 3600.0;

#[derive(Debug, Serialize, Deserialize)]
pub struct SceneManifest {
    pub schema_version: u32,
    pub scene_id: String,
    pub sample_rate: u32,
    pub channels: u16,
    pub total_frames: u64,
    pub dialogue: Vec<DialogueManifest>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct DialogueManifest {
    pub actor: String,
    pub start_frame: u64,
    pub duration_frames: u64,
    pub trim_start_frame: u64,
    pub gain: f32,
    /// Provenance does not imply perceptual alignment validation.
    pub alignment_source: Option<AlignmentSource>,
    /// Half-open scene sample intervals; gaps evaluate to Rest.
    pub visemes: Vec<SampleViseme>,
}
#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct SampleViseme {
    pub start_frame: u64,
    pub end_frame: u64,
    pub viseme: Viseme,
}
pub struct RenderedScene {
    pub audio: AudioBuffer,
    pub manifest: SceneManifest,
}

/// Rejects scene overflow instead of silently truncating dialogue in the mixer.
/// Scene duration rounds to the nearest sample, exactly as dialogue placement does.
pub fn render_scene(
    scene_id: &str,
    duration_seconds: f64,
    clips: &[DialogueClip],
) -> Result<RenderedScene, SpeechError> {
    if scene_id.trim().is_empty()
        || !duration_seconds.is_finite()
        || !(0.0..=MAX_SCENE_SECONDS).contains(&duration_seconds)
    {
        return Err(SpeechError(
            "export requires a scene ID and finite duration within one hour".into(),
        ));
    }
    let total_frames = (duration_seconds * SAMPLE_RATE as f64).round() as u64;
    let mut dialogue = Vec::with_capacity(clips.len());
    for clip in clips {
        clip.audio
            .validate()
            .map_err(|e| SpeechError(e.to_string()))?;
        if clip.audio.sample_rate != SAMPLE_RATE
            || clip.audio.channels != 1
            || !clip.gain.is_finite()
        {
            return Err(SpeechError(
                "export dialogue must be 24kHz mono with finite gain".into(),
            ));
        }
        let duration_frames = (clip.audio.frames() as u64)
            .checked_sub(clip.trim_start_frame)
            .ok_or_else(|| SpeechError("export trim exceeds source duration".into()))?;
        let end = clip
            .start_frame
            .checked_add(duration_frames)
            .ok_or_else(|| SpeechError("export dialogue position overflow".into()))?;
        if end > total_frames {
            return Err(SpeechError(format!(
                "{} dialogue exceeds scene; adjust timing or duration",
                clip.actor
            )));
        }
        let mut visemes = Vec::new();
        let alignment_source = if let Some(alignment) = &clip.alignment {
            alignment
                .validate()
                .map_err(|e| SpeechError(e.to_string()))?;
            if (alignment.duration_seconds - clip.audio.duration_seconds()).abs()
                > 1.0 / SAMPLE_RATE as f64
            {
                return Err(SpeechError(
                    "alignment duration differs from export audio".into(),
                ));
            }
            for interval in alignment
                .visemes()
                .map_err(|e| SpeechError(e.to_string()))?
            {
                // First integer sample whose time is >= the boundary, matching mouth_at.
                let start = (interval.start_seconds * SAMPLE_RATE as f64).ceil() as u64;
                let end = (interval.end_seconds * SAMPLE_RATE as f64).ceil() as u64;
                let start = start.max(clip.trim_start_frame);
                let end = end.min(clip.audio.frames() as u64);
                if start < end {
                    visemes.push(SampleViseme {
                        start_frame: clip.start_frame + start - clip.trim_start_frame,
                        end_frame: clip.start_frame + end - clip.trim_start_frame,
                        viseme: interval.viseme,
                    });
                }
            }
            Some(alignment.source.clone())
        } else {
            None
        };
        dialogue.push(DialogueManifest {
            actor: clip.actor.clone(),
            start_frame: clip.start_frame,
            duration_frames,
            trim_start_frame: clip.trim_start_frame,
            gain: clip.gain,
            alignment_source,
            visemes,
        });
    }
    let mixed = motion_audio::mix_timeline(
        &clips.iter().map(DialogueClip::mix_clip).collect::<Vec<_>>(),
        SAMPLE_RATE,
        1,
        total_frames as usize,
    )
    .map_err(|e| SpeechError(e.to_string()))?;
    Ok(RenderedScene {
        audio: mixed,
        manifest: SceneManifest {
            schema_version: 1,
            scene_id: scene_id.into(),
            sample_rate: SAMPLE_RATE,
            channels: 1,
            total_frames,
            dialogue,
        },
    })
}
