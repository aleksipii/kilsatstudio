//! Prepared audio and mouth tracks evaluated against one sample clock.
use crate::DialogueJob;
use motion_audio::{AudioBuffer, MixClip};
use motion_lipsync::cache::{AlignmentCache, AlignmentInput, AlignmentStatus};
use motion_lipsync::{Alignment, Viseme};
use motion_tts::{SpeechError, SpeechResult};

pub struct DialogueClip {
    pub actor: String,
    pub start_frame: u64,
    pub audio: AudioBuffer,
    pub alignment: Option<Alignment>,
    pub estimated_alignment: Option<motion_lipsync::rhubarb::EstimatedVisemeAlignment>,
    pub gain: f32,
    pub trim_start_frame: u64,
}
impl DialogueClip {
    pub fn prepare(
        job: &DialogueJob,
        result: &SpeechResult,
        alignment: Option<Alignment>,
    ) -> Result<Self, SpeechError> {
        let audio =
            motion_audio::read_wav(&result.audio_path).map_err(|e| SpeechError(e.to_string()))?;
        Self::from_audio(job, audio, alignment)
    }
    /// Resolves imported or cached alignment against decoded PCM and the current text.
    pub fn prepare_with_alignment_input(
        job: &DialogueJob,
        result: &SpeechResult,
        input: Option<&AlignmentInput>,
        cache: &AlignmentCache,
    ) -> Result<(Self, AlignmentStatus), SpeechError> {
        let audio =
            motion_audio::read_wav(&result.audio_path).map_err(|e| SpeechError(e.to_string()))?;
        let mut clip = Self::from_audio(job, audio, None)?;
        let resolved = match input {
            Some(input) => Some(
                input
                    .resolve(&clip.audio, &job.request.text, cache)
                    .map_err(|e| SpeechError(e.to_string()))?,
            ),
            None => None,
        };
        let (alignment, estimated, status) = match resolved {
            Some(resolved) => (resolved.alignment, resolved.estimated, resolved.status),
            None => (None, None, AlignmentStatus::None),
        };
        clip.alignment = alignment;
        clip.estimated_alignment = estimated;
        Ok((clip, status))
    }
    fn from_audio(
        job: &DialogueJob,
        audio: AudioBuffer,
        alignment: Option<Alignment>,
    ) -> Result<Self, SpeechError> {
        if audio.sample_rate != 24000 || audio.channels != 1 {
            return Err(SpeechError("dialogue must be 24kHz mono".into()));
        }
        if let Some(a) = &alignment {
            a.validate().map_err(|e| SpeechError(e.to_string()))?;
            if (a.duration_seconds - audio.duration_seconds()).abs() > 1.0 / 24000.0 {
                return Err(SpeechError("alignment duration differs from audio".into()));
            }
        }
        if !job.at.is_finite() || job.at < 0.0 || job.at * 24000.0 >= u64::MAX as f64 {
            return Err(SpeechError("invalid timeline position".into()));
        }
        Ok(Self {
            actor: job.actor.clone(),
            start_frame: (job.at * 24000.0).round() as u64,
            audio,
            alignment,
            estimated_alignment: None,
            gain: 1.0,
            trim_start_frame: 0,
        })
    }
    /// Both preview and export call this with the audio device/export sample clock.
    pub fn mouth_at(&self, scene_frame: u64) -> Result<Viseme, SpeechError> {
        if scene_frame < self.start_frame {
            return Ok(Viseme::Rest);
        }
        let local = scene_frame - self.start_frame;
        let local = local.saturating_add(self.trim_start_frame);
        if local >= self.audio.frames() as u64 {
            return Ok(Viseme::Rest);
        }
        if let Some(estimated) = &self.estimated_alignment {
            if self.alignment.is_some() {
                return Err(SpeechError(
                    "conflicting phoneme and estimated mouth tracks".into(),
                ));
            }
            return estimated
                .viseme_at_sample(local)
                .map_err(|e| SpeechError(e.to_string()));
        }
        match &self.alignment {
            Some(a) => a
                .viseme_at(local as f64 / self.audio.sample_rate as f64)
                .map_err(|e| SpeechError(e.to_string())),
            None => Ok(Viseme::Rest),
        }
    }
    pub fn mix_clip(&self) -> MixClip {
        MixClip {
            audio: self.audio.clone(),
            start_frame: self.start_frame,
            trim_start_frame: self.trim_start_frame,
            duration_frames: None,
            gain: self.gain,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use motion_lipsync::{AlignmentSource, TimedPhoneme};
    #[test]
    fn audio_and_mouth_share_seek_trim_and_start_clock() {
        let clip = DialogueClip {
            actor: "EMMA".into(),
            start_frame: 24000,
            audio: AudioBuffer {
                sample_rate: 24000,
                channels: 1,
                samples: vec![0.25; 24000],
            },
            estimated_alignment: None,
            alignment: Some(Alignment {
                source: AlignmentSource::Manual,
                duration_seconds: 1.0,
                phonemes: vec![TimedPhoneme {
                    phoneme: "m".into(),
                    start_seconds: 0.0,
                    end_seconds: 0.5,
                }],
            }),
            gain: 1.0,
            trim_start_frame: 0,
        };
        assert_eq!(clip.mouth_at(23999).unwrap(), Viseme::Rest);
        assert_eq!(clip.mouth_at(24000).unwrap(), Viseme::Closed);
        assert_eq!(clip.mouth_at(36000).unwrap(), Viseme::Rest);
        let mix = motion_audio::mix_timeline(&[clip.mix_clip()], 24000, 1, 48000).unwrap();
        assert_eq!(mix.samples[23999], 0.0);
        assert_eq!(mix.samples[24000], 0.25);
        let trimmed = DialogueClip {
            trim_start_frame: 12000,
            ..clip
        };
        assert_eq!(trimmed.mouth_at(24000).unwrap(), Viseme::Rest);
    }
}
