//! Prepared audio and mouth tracks evaluated against one sample clock.
use crate::DialogueJob;
use motion_audio::{AudioBuffer, MixClip};
use motion_lipsync::cache::{AlignmentCache, AlignmentInput, AlignmentStatus};
use motion_lipsync::english::{
    estimate_identity_in_span, estimate_viseme_track_in_span, pronunciation_report, EnglishError,
    EstimatedVisemeTrack, Pronunciations,
};
use motion_lipsync::timing_eval::{energy_envelope, EnvelopeConfig};
use motion_lipsync::{Alignment, AlignmentSource, Viseme};
use motion_tts::{SpeechError, SpeechResult};

/// Mouth track estimated from the dialogue text alone (never forced alignment).
#[derive(Clone, Debug, PartialEq)]
pub struct TextEstimate {
    pub track: EstimatedVisemeTrack,
    /// `estimate_identity_in_span` of text, overrides, lexicon version, audio length and
    /// speech span. A stored estimate is stale when this no longer matches.
    pub identity: String,
}

/// Where a clip's mouth shapes come from, in precedence order (highest first).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouthSource {
    /// Hand authored against the waveform.
    Manual,
    /// External aligner output.
    Forced,
    /// Rhubarb recognizer cues (estimated, derived from audio).
    RhubarbEstimated,
    /// Built-in English estimate from the text only; does not hear the audio.
    TextEstimated,
    /// No mouth data: mouth stays at rest.
    None,
}

pub struct DialogueClip {
    pub actor: String,
    pub start_frame: u64,
    pub audio: AudioBuffer,
    pub alignment: Option<Alignment>,
    pub estimated_alignment: Option<motion_lipsync::rhubarb::EstimatedVisemeAlignment>,
    /// Lowest precedence: used only when `alignment` and `estimated_alignment` are None.
    pub text_estimated: Option<TextEstimate>,
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
        let start_frame = motion_audio::clock::seconds_to_sample(job.at)
            .map_err(|_| SpeechError("invalid timeline position".into()))?;
        Ok(Self {
            actor: job.actor.clone(),
            start_frame,
            audio,
            alignment,
            estimated_alignment: None,
            text_estimated: None,
            gain: 1.0,
            trim_start_frame: 0,
        })
    }
    /// Highest-precedence mouth source present. Precedence: manual > forced-aligned >
    /// Rhubarb estimated > text estimated.
    pub fn mouth_source(&self) -> MouthSource {
        match (
            &self.alignment,
            &self.estimated_alignment,
            &self.text_estimated,
        ) {
            (Some(a), _, _) => match a.source {
                AlignmentSource::Manual => MouthSource::Manual,
                AlignmentSource::Forced { .. } => MouthSource::Forced,
                // Legacy bare estimated phoneme alignments rank with recognizer cues.
                AlignmentSource::Estimated { .. } => MouthSource::RhubarbEstimated,
            },
            (None, Some(_), _) => MouthSource::RhubarbEstimated,
            (None, None, Some(_)) => MouthSource::TextEstimated,
            (None, None, None) => MouthSource::None,
        }
    }
    /// Derives a text estimate scaled to the clip's detected speech span (see
    /// [`speech_span`]; the mouth rests over leading and trailing silence), but only when
    /// no manual, forced or Rhubarb alignment exists. Replaces a stale estimate. Returns
    /// the text-estimate outcome so callers can report low confidence words.
    pub fn derive_text_estimate(
        &mut self,
        text: &str,
        overrides: &Pronunciations,
    ) -> Result<Option<TextEstimateInfo>, EnglishError> {
        if self.alignment.is_some() || self.estimated_alignment.is_some() {
            self.text_estimated = None;
            return Ok(None);
        }
        let frames = self.audio.frames() as u64;
        let (start, end) = speech_span(&self.audio);
        let identity = estimate_identity_in_span(text, overrides, frames, start, end);
        if let Some(existing) = &self.text_estimated {
            if existing.identity == identity {
                return Ok(Some(TextEstimateInfo::from_track(
                    &existing.track,
                    text,
                    overrides,
                )));
            }
        }
        self.text_estimated = None;
        let track = estimate_viseme_track_in_span(text, overrides, frames, start, end)?;
        let info = TextEstimateInfo::from_track(&track, text, overrides);
        self.text_estimated = Some(TextEstimate { track, identity });
        Ok(Some(info))
    }
    /// Finnish text estimate; only measured leading/trailing silence constrains timing.
    pub fn derive_finnish_estimate(
        &mut self,
        text: &str,
    ) -> Result<Option<TextEstimateInfo>, SpeechError> {
        if self.alignment.is_some() || self.estimated_alignment.is_some() {
            self.text_estimated = None;
            return Ok(None);
        }
        self.text_estimated = None;
        let frames = self.audio.frames() as u64;
        let span = energy_envelope(&self.audio.samples, EnvelopeConfig::DEFAULT_24K).speech_span();
        let has_speech = span.is_some();
        let (start, end) = span.map_or((0, 0), |s| (s.start, s.end));
        let track = if has_speech {
            motion_lipsync::finnish::estimate(text, frames, start, end)
        } else {
            motion_lipsync::finnish::silent_track(frames)
        }
        .map_err(|e| SpeechError(e.to_string()))?;
        let info = TextEstimateInfo {
            low_confidence_words: Vec::new(),
            time_scale: track.time_scale,
        };
        self.text_estimated = Some(TextEstimate {
            track,
            identity: motion_lipsync::finnish::identity(text, frames, start, end),
        });
        Ok(Some(info))
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
        match (&self.alignment, &self.text_estimated) {
            (Some(a), _) => a
                .viseme_at(local as f64 / self.audio.sample_rate as f64)
                .map_err(|e| SpeechError(e.to_string())),
            (None, Some(t)) => Ok(t.track.viseme_at_sample(local)),
            (None, None) => Ok(Viseme::Rest),
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

/// Speech span `[start, end)` used to place a text estimate in a clip: the first to
/// last above-threshold sample of the RMS envelope with
/// [`EnvelopeConfig::DEFAULT_24K`] (threshold max(-50 dBFS, peak - 35 dB)). Covers
/// the whole clip when nothing exceeds the threshold, so a silent or very quiet clip
/// keeps the previous whole-clip behaviour. Deterministic in the PCM samples. See
/// docs/lipsync-timing-evaluation.md for the measurements behind this choice.
pub fn speech_span(audio: &AudioBuffer) -> (u64, u64) {
    let frames = audio.frames() as u64;
    match energy_envelope(&audio.samples, EnvelopeConfig::DEFAULT_24K).speech_span() {
        Some(span) if audio.channels == 1 => (span.start, span.end),
        _ => (0, frames),
    }
}

/// Facts about a derived estimate that callers may surface as diagnostics.
#[derive(Clone, Debug, PartialEq)]
pub struct TextEstimateInfo {
    /// Sorted, deduplicated words pronounced by the rule-based fallback.
    pub low_confidence_words: Vec<String>,
    /// Audio length over unscaled model length (1.0 means the model rate fits).
    pub time_scale: f64,
}
impl TextEstimateInfo {
    fn from_track(track: &EstimatedVisemeTrack, text: &str, overrides: &Pronunciations) -> Self {
        Self {
            low_confidence_words: pronunciation_report(text, overrides)
                .low_confidence_words()
                .into_iter()
                .map(str::to_owned)
                .collect(),
            time_scale: track.time_scale,
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
            text_estimated: None,
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
