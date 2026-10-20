//! Non-speech audio cues placed on the same 24 kHz scene sample clock as dialogue.
//! Cues are decoded once off the UI thread and mixed by `motion_audio::mix_timeline`
//! together with dialogue, so preview and export share one clock and one mixer.
use crate::export::SAMPLE_RATE;
use motion_audio::{AudioBuffer, MixClip};
use motion_script::{AudioCue, Scene};
use motion_tts::{CancellationToken, SpeechError};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Total source samples decoded for one scene's cues (5 minutes at 24 kHz, about 29 MB as f32).
pub const MAX_CUE_SOURCE_FRAMES: u64 = SAMPLE_RATE as u64 * 300;

#[derive(Clone, Debug)]
pub struct CueClip {
    pub id: String,
    pub asset: String,
    pub start_frame: u64,
    pub trim_start_frame: u64,
    /// Effective playback length after source and scene-end truncation.
    pub duration_frames: u64,
    pub gain: f32,
    pub audio: AudioBuffer,
}
impl CueClip {
    pub fn mix_clip(&self) -> MixClip {
        MixClip {
            audio: self.audio.clone(),
            start_frame: self.start_frame,
            trim_start_frame: self.trim_start_frame,
            duration_frames: Some(self.duration_frames),
            gain: self.gain,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CueManifest {
    pub id: String,
    pub asset: String,
    pub start_frame: u64,
    pub duration_frames: u64,
    pub trim_start_frame: u64,
    pub gain: f32,
}

fn frames(seconds: f64) -> u64 {
    (seconds * SAMPLE_RATE as f64).round() as u64
}

/// Builds a clip from decoded PCM. Placement, trim and duration use the dialogue rounding.
/// Playback is truncated at the scene end; a cue that starts at or after it is an error.
pub fn build_clip(
    cue: &AudioCue,
    audio: AudioBuffer,
    scene_frames: u64,
) -> Result<CueClip, SpeechError> {
    let fail = |m: &str| SpeechError(format!("audio cue {} ({}): {m}", cue.id, cue.asset));
    audio.validate().map_err(|e| fail(&e.to_string()))?;
    if audio.sample_rate != SAMPLE_RATE || audio.channels != 1 {
        return Err(fail("requires 24kHz mono audio; convert before import"));
    }
    if !cue.at.is_finite()
        || cue.at < 0.0
        || !cue.gain.is_finite()
        || !(0.0..=4.0).contains(&cue.gain)
    {
        return Err(fail("invalid time or gain"));
    }
    let start = frames(cue.at);
    if start >= scene_frames {
        return Err(fail("starts at or after the scene end"));
    }
    let trim = frames(cue.trim_start);
    let source = audio.frames() as u64;
    if trim >= source {
        return Err(fail("trim_start leaves no audio"));
    }
    let mut length = source - trim;
    if let Some(d) = cue.duration {
        length = length.min(frames(d).max(1));
    }
    length = length.min(scene_frames - start);
    Ok(CueClip {
        id: cue.id.clone(),
        asset: cue.asset.clone(),
        start_frame: start,
        trim_start_frame: trim,
        duration_frames: length,
        gain: cue.gain as f32,
        audio,
    })
}

/// Decodes every cue of a scene with bounded work: header lengths are checked before
/// decoding, and assets must stay inside the project root (symlinks included).
pub fn prepare_cues(
    scene: &Scene,
    root: &Path,
    cancel: &CancellationToken,
) -> Result<Vec<CueClip>, SpeechError> {
    let cues = &scene.composition.audio_cues;
    if cues.is_empty() {
        return Ok(Vec::new());
    }
    let scene_frames = frames(scene.duration);
    let root = root
        .canonicalize()
        .map_err(|e| SpeechError(format!("audio cue project root: {e}")))?;
    let mut budget = 0u64;
    let mut clips = Vec::with_capacity(cues.len());
    for cue in cues {
        cancel.check()?;
        let context = |m: String| {
            SpeechError(format!(
                "scene {} audio cue {} ({}): {m}",
                scene.id, cue.id, cue.asset
            ))
        };
        let path = root
            .join(&cue.asset)
            .canonicalize()
            .map_err(|e| context(format!("invalid asset reference: {e}")))?;
        if !path.starts_with(&root) || !path.is_file() {
            return Err(context(
                "invalid asset reference: not a file inside the project".into(),
            ));
        }
        let reader = hound::WavReader::open(&path).map_err(|e| context(format!("import: {e}")))?;
        let spec = reader.spec();
        if spec.channels != 1
            || spec.sample_rate != SAMPLE_RATE
            || spec.bits_per_sample != 16
            || spec.sample_format != hound::SampleFormat::Int
            || reader.duration() == 0
        {
            return Err(context(
                "requires nonempty 24kHz mono PCM16 WAV; convert before import".into(),
            ));
        }
        budget = budget.saturating_add(reader.duration() as u64);
        if budget > MAX_CUE_SOURCE_FRAMES {
            return Err(context("scene cue decoded-audio budget exceeded".into()));
        }
        drop(reader);
        let audio = motion_audio::read_wav(&path).map_err(|e| context(e.to_string()))?;
        clips.push(build_clip(cue, audio, scene_frames).map_err(|e| context(e.0))?);
    }
    Ok(clips)
}

pub fn manifest(clips: &[CueClip]) -> Vec<CueManifest> {
    clips
        .iter()
        .map(|c| CueManifest {
            id: c.id.clone(),
            asset: c.asset.clone(),
            start_frame: c.start_frame,
            duration_frames: c.duration_frames,
            trim_start_frame: c.trim_start_frame,
            gain: c.gain,
        })
        .collect()
}
