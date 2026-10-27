//! AudioTimeline v1 (ORK-G1-04): where each dialogue clip comes from, where it sits on
//! the 24 kHz scene clock and which mouth timing it carries, with its provenance.
//!
//! Positions are whole samples on `motion_audio::clock`. Placement uses the same rules
//! as `motion_audio::MixClip`: the clip plays source samples
//! `[trim_start_samples, trim_start_samples + duration_samples)` starting at scene
//! sample `start_sample`. Phoneme and viseme intervals are half-open and in
//! *source-local* samples (sample 0 is the first sample of the untrimmed source), so a
//! trim edit never moves the mouth relative to the audio. Gaps evaluate to `Rest`.
//!
//! Structural validation only: a valid timeline does not mean the mouth timing has
//! been measured against the audio.
use crate::{english::VisemeCue, phoneme_to_viseme, AlignmentSource, Viseme};
use motion_audio::{clock::DIALOGUE_SAMPLE_RATE, AudioBuffer, MixClip};
use serde::{Deserialize, Serialize};
use std::fmt;

pub const AUDIO_TIMELINE_FORMAT_VERSION: u32 = 1;
pub const MAX_TIMELINE_CLIPS: usize = 4096;
pub const MAX_CLIP_INTERVALS: usize = 65_536;
const MAX_LABEL_BYTES: usize = 256;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioTimeline {
    pub format_version: u32,
    pub scene_id: String,
    pub sample_rate: u32,
    /// Scene length in samples; every clip ends at or before it.
    pub duration_samples: u64,
    pub clips: Vec<AudioTimelineClip>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioTimelineClip {
    /// Stable dialogue ID (`motion_voice::dialogue_jobs` order).
    pub id: u64,
    pub actor: String,
    /// BCP 47 style tag as written in the screenplay (`fi-fi`, `en-us`, …).
    pub language: String,
    pub source: AudioSourceInfo,
    /// Length of the untrimmed source audio in samples.
    pub source_samples: u64,
    pub start_sample: u64,
    pub trim_start_samples: u64,
    pub duration_samples: u64,
    pub gain: f32,
    /// Absent when the clip has no mouth timing (the mouth rests).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mouth: Option<MouthTiming>,
}

/// Origin of the PCM. Recorded as data; this crate never calls a speech provider.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum AudioSourceInfo {
    /// A WAV the user supplied. `path` is relative to the project root.
    Imported { path: String, pcm_sha256: String },
    /// Synthesized ahead of time by a local provider (never at frame evaluation).
    Synthesized {
        provider: String,
        model_revision: String,
        voice: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        voice_revision: Option<String>,
        cache_key: String,
        pcm_sha256: String,
    },
}
impl AudioSourceInfo {
    pub fn pcm_sha256(&self) -> &str {
        match self {
            Self::Imported { pcm_sha256, .. } | Self::Synthesized { pcm_sha256, .. } => pcm_sha256,
        }
    }
    pub fn is_synthesized(&self) -> bool {
        matches!(self, Self::Synthesized { .. })
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MouthTiming {
    /// Manual, forced or estimated. Kept verbatim through save, reopen and export.
    pub source: AlignmentSource,
    /// Optional IPA phoneme intervals the visemes came from.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub phonemes: Vec<SamplePhoneme>,
    pub visemes: Vec<VisemeCue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SamplePhoneme {
    pub phoneme: String,
    pub start_sample: u64,
    pub end_sample: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AudioTimelineError {
    UnsupportedVersion(u32),
    UnsupportedSampleRate(u32),
    InvalidLabel(&'static str),
    TooManyClips(usize),
    DuplicateClipId(u64),
    /// Trim/duration exceed the source, or the clip ends after the scene.
    Placement {
        id: u64,
        reason: &'static str,
    },
    InvalidGain(u64),
    InvalidProvenance {
        id: u64,
        reason: &'static str,
    },
    /// Mouth intervals are unordered, overlapping, empty or outside the source.
    MouthIntervals {
        id: u64,
        reason: String,
    },
    OverlappingActorClips {
        actor: String,
        first: u64,
        second: u64,
    },
    /// The supplied PCM does not match the clip's recorded source.
    AudioMismatch {
        id: u64,
        reason: &'static str,
    },
    InvalidTransition,
}
impl fmt::Display for AudioTimelineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedVersion(v) => {
                write!(
                    f,
                    "unsupported AudioTimeline format_version {v}; supported is 1"
                )
            }
            Self::UnsupportedSampleRate(r) => {
                write!(
                    f,
                    "AudioTimeline sample rate {r} Hz; the dialogue clock is 24000 Hz"
                )
            }
            Self::InvalidLabel(what) => write!(f, "AudioTimeline {what} must be 1..256 bytes"),
            Self::TooManyClips(n) => write!(f, "{n} clips exceed the {MAX_TIMELINE_CLIPS} limit"),
            Self::DuplicateClipId(id) => write!(f, "dialogue ID {id} appears twice"),
            Self::Placement { id, reason } => write!(f, "dialogue {id}: {reason}"),
            Self::InvalidGain(id) => write!(f, "dialogue {id}: gain must be finite"),
            Self::InvalidProvenance { id, reason } => write!(f, "dialogue {id}: {reason}"),
            Self::MouthIntervals { id, reason } => {
                write!(f, "dialogue {id} mouth timing: {reason}")
            }
            Self::OverlappingActorClips {
                actor,
                first,
                second,
            } => write!(
                f,
                "{actor}: dialogues {first} and {second} overlap; one mouth cannot say both"
            ),
            Self::AudioMismatch { id, reason } => write!(f, "dialogue {id}: {reason}"),
            Self::InvalidTransition => f.write_str("viseme transition length is too large"),
        }
    }
}
impl std::error::Error for AudioTimelineError {}

fn label(value: &str, what: &'static str) -> Result<(), AudioTimelineError> {
    if value.trim().is_empty() || value.len() > MAX_LABEL_BYTES || value.contains('\0') {
        return Err(AudioTimelineError::InvalidLabel(what));
    }
    Ok(())
}
fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Interpolation state between two mouth shapes. `weight` 0 shows `from`, 1 shows `to`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisemeBlend {
    pub from: Viseme,
    pub to: Viseme,
    pub weight: f32,
}
impl VisemeBlend {
    pub fn hold(viseme: Viseme) -> Self {
        Self {
            from: viseme,
            to: viseme,
            weight: 0.0,
        }
    }
    /// The shape with the larger weight; ties at 0.5 show `to`.
    pub fn dominant(&self) -> Viseme {
        if self.weight >= 0.5 {
            self.to
        } else {
            self.from
        }
    }
}

impl AudioTimelineClip {
    pub fn end_sample(&self) -> u64 {
        self.start_sample + self.duration_samples
    }

    fn validate(&self, scene_samples: u64) -> Result<(), AudioTimelineError> {
        let id = self.id;
        label(&self.actor, "actor")?;
        label(&self.language, "language")?;
        let placement = |reason| AudioTimelineError::Placement { id, reason };
        if self.source_samples == 0 || self.duration_samples == 0 {
            return Err(placement("source and duration must be at least one sample"));
        }
        match self.trim_start_samples.checked_add(self.duration_samples) {
            Some(end) if end <= self.source_samples => {}
            _ => return Err(placement("trim and duration exceed the source audio")),
        }
        match self.start_sample.checked_add(self.duration_samples) {
            Some(end) if end <= scene_samples => {}
            _ => {
                return Err(placement(
                    "clip ends after the scene; adjust timing or duration",
                ))
            }
        }
        if !self.gain.is_finite() {
            return Err(AudioTimelineError::InvalidGain(id));
        }
        let provenance = |reason| AudioTimelineError::InvalidProvenance { id, reason };
        if !is_sha256(self.source.pcm_sha256()) {
            return Err(provenance("pcm_sha256 must be 64 lowercase hex digits"));
        }
        match &self.source {
            AudioSourceInfo::Imported { path, .. } => {
                if path.trim().is_empty() || path.contains('\0') {
                    return Err(provenance("imported audio needs a path"));
                }
            }
            AudioSourceInfo::Synthesized {
                provider,
                model_revision,
                voice,
                voice_revision,
                cache_key,
                ..
            } => {
                for value in [provider, model_revision, voice, cache_key]
                    .into_iter()
                    .chain(voice_revision)
                {
                    if value.trim().is_empty() || value.len() > MAX_LABEL_BYTES {
                        return Err(provenance(
                            "synthesized audio needs provider, model revision, voice and cache key",
                        ));
                    }
                }
            }
        }
        if let Some(mouth) = &self.mouth {
            mouth.validate(id, self.source_samples)?;
        }
        Ok(())
    }

    /// Source-local sample shown at scene sample `sample`, if the clip is playing.
    pub fn source_sample_at(&self, sample: u64) -> Option<u64> {
        (self.start_sample..self.end_sample())
            .contains(&sample)
            .then(|| sample - self.start_sample + self.trim_start_samples)
    }

    /// Mixer placement for decoded PCM. Fails when the PCM is not the recorded source.
    pub fn mix_clip(&self, audio: AudioBuffer) -> Result<MixClip, AudioTimelineError> {
        let mismatch = |reason| AudioTimelineError::AudioMismatch {
            id: self.id,
            reason,
        };
        if audio.sample_rate != DIALOGUE_SAMPLE_RATE || audio.channels != 1 {
            return Err(mismatch("audio must be 24 kHz mono"));
        }
        if audio.frames() as u64 != self.source_samples {
            return Err(mismatch("audio length differs from source_samples"));
        }
        Ok(MixClip {
            audio,
            start_frame: self.start_sample,
            trim_start_frame: self.trim_start_samples,
            duration_frames: Some(self.duration_samples),
            gain: self.gain,
        })
    }

    /// Mouth shape at scene sample `sample`; `Rest` outside the clip or without timing.
    pub fn viseme_at(&self, sample: u64) -> Viseme {
        match (&self.mouth, self.source_sample_at(sample)) {
            (Some(mouth), Some(local)) => mouth.viseme_at(local),
            _ => Viseme::Rest,
        }
    }

    /// Blended mouth at scene sample `sample`. Each change of shape inside the visible
    /// window crossfades over `[b - h, b + h)` around the boundary `b`, where
    /// `h = min(transition_samples / 2, half of each neighbouring segment)`. The clip's
    /// own (possibly trimmed) edges are hard cuts, like its audio: no crossfade reaches
    /// outside the visible window. `transition_samples == 0` is a hard cut everywhere.
    pub fn blend_at(&self, sample: u64, transition_samples: u64) -> VisemeBlend {
        let Some(local) = self.source_sample_at(sample) else {
            return VisemeBlend::hold(Viseme::Rest);
        };
        let lo = self.trim_start_samples;
        let hi = self.trim_start_samples + self.duration_samples;
        let segment = |at: u64| -> (u64, u64, Viseme) {
            let (start, end, viseme) = match &self.mouth {
                Some(mouth) => mouth.segment_at(at, self.source_samples),
                None => (0, self.source_samples, Viseme::Rest),
            };
            (start.max(lo), end.min(hi), viseme)
        };
        let (start, end, current) = segment(local);
        let half = transition_samples / 2;
        let len = end - start;
        // Window around the boundary at `start` (from the previous segment).
        let (prev_len, previous) = if start > lo {
            let (s, e, v) = segment(start - 1);
            (e - s, v)
        } else {
            (0, current)
        };
        let h = half.min(len / 2).min(prev_len / 2);
        if previous != current && h > 0 && local < start + h {
            let weight = (local + h - start) as f64 / (2 * h) as f64;
            return VisemeBlend {
                from: previous,
                to: current,
                weight: weight as f32,
            };
        }
        // Window around the boundary at `end` (into the next segment).
        let (next_len, next) = if end < hi {
            let (s, e, v) = segment(end);
            (e - s, v)
        } else {
            (0, current)
        };
        let h = half.min(len / 2).min(next_len / 2);
        if next != current && h > 0 && local >= end - h {
            let weight = (local + h - end) as f64 / (2 * h) as f64;
            return VisemeBlend {
                from: current,
                to: next,
                weight: weight as f32,
            };
        }
        VisemeBlend::hold(current)
    }
}

impl MouthTiming {
    fn validate(&self, id: u64, source_samples: u64) -> Result<(), AudioTimelineError> {
        let fail = |reason: String| AudioTimelineError::MouthIntervals { id, reason };
        if let AlignmentSource::Forced { tool, version }
        | AlignmentSource::Estimated { tool, version } = &self.source
        {
            if tool.trim().is_empty() || version.trim().is_empty() {
                return Err(fail(
                    "forced or estimated timing needs tool and version".into(),
                ));
            }
        }
        if self.visemes.len() > MAX_CLIP_INTERVALS || self.phonemes.len() > MAX_CLIP_INTERVALS {
            return Err(fail(format!("more than {MAX_CLIP_INTERVALS} intervals")));
        }
        let mut previous = 0;
        for cue in &self.visemes {
            if cue.start_sample < previous
                || cue.end_sample <= cue.start_sample
                || cue.end_sample > source_samples
            {
                return Err(fail(format!(
                    "viseme [{}, {}) must be ordered, non-empty, non-overlapping and within the source",
                    cue.start_sample, cue.end_sample
                )));
            }
            previous = cue.end_sample;
        }
        let mut previous = 0;
        for p in &self.phonemes {
            if p.start_sample < previous
                || p.end_sample <= p.start_sample
                || p.end_sample > source_samples
            {
                return Err(fail(format!(
                    "phoneme {:?} [{}, {}) must be ordered, non-empty and within the source",
                    p.phoneme, p.start_sample, p.end_sample
                )));
            }
            if phoneme_to_viseme(&p.phoneme).is_none() {
                return Err(fail(format!("unsupported IPA phoneme {:?}", p.phoneme)));
            }
            previous = p.end_sample;
        }
        Ok(())
    }

    /// Viseme at a source-local sample (half-open; gaps are `Rest`).
    pub fn viseme_at(&self, local: u64) -> Viseme {
        let i = self.visemes.partition_point(|c| c.end_sample <= local);
        self.visemes
            .get(i)
            .filter(|c| c.start_sample <= local)
            .map_or(Viseme::Rest, |c| c.viseme)
    }

    /// Maximal constant-shape segment containing `local`, with gaps filled by `Rest`
    /// and adjacent equal shapes merged. Bounded by `[0, source_samples)`.
    fn segment_at(&self, local: u64, source_samples: u64) -> (u64, u64, Viseme) {
        let cues = &self.visemes;
        let shape = self.viseme_at(local);
        // `local` lies in cue i or in the gap before cue i.
        let i = cues.partition_point(|c| c.end_sample <= local);
        let inside = cues.get(i).is_some_and(|c| c.start_sample <= local);
        let (mut start, mut end) = if inside {
            (cues[i].start_sample, cues[i].end_sample)
        } else {
            let gap_start = if i == 0 { 0 } else { cues[i - 1].end_sample };
            let gap_end = cues.get(i).map_or(source_samples, |c| c.start_sample);
            (gap_start, gap_end)
        };
        // Extend over neighbours with the same shape (cues or Rest gaps).
        while start > 0 && self.viseme_at(start - 1) == shape {
            let j = cues.partition_point(|c| c.end_sample < start);
            start = match cues.get(j).filter(|c| c.start_sample < start) {
                Some(c) => c.start_sample,
                None => {
                    if j == 0 {
                        0
                    } else {
                        cues[j - 1].end_sample
                    }
                }
            };
        }
        while end < source_samples && self.viseme_at(end) == shape {
            let j = cues.partition_point(|c| c.end_sample <= end);
            end = match cues.get(j).filter(|c| c.start_sample <= end) {
                Some(c) => c.end_sample,
                None => cues.get(j).map_or(source_samples, |c| c.start_sample),
            };
        }
        (start, end.min(source_samples), shape)
    }
}

impl AudioTimeline {
    pub fn new(scene_id: impl Into<String>, duration_samples: u64) -> Self {
        Self {
            format_version: AUDIO_TIMELINE_FORMAT_VERSION,
            scene_id: scene_id.into(),
            sample_rate: DIALOGUE_SAMPLE_RATE,
            duration_samples,
            clips: Vec::new(),
        }
    }

    pub fn validate(&self) -> Result<(), AudioTimelineError> {
        if self.format_version != AUDIO_TIMELINE_FORMAT_VERSION {
            return Err(AudioTimelineError::UnsupportedVersion(self.format_version));
        }
        if self.sample_rate != DIALOGUE_SAMPLE_RATE {
            return Err(AudioTimelineError::UnsupportedSampleRate(self.sample_rate));
        }
        label(&self.scene_id, "scene_id")?;
        if self.clips.len() > MAX_TIMELINE_CLIPS {
            return Err(AudioTimelineError::TooManyClips(self.clips.len()));
        }
        let mut ids = std::collections::BTreeSet::new();
        for clip in &self.clips {
            if !ids.insert(clip.id) {
                return Err(AudioTimelineError::DuplicateClipId(clip.id));
            }
            clip.validate(self.duration_samples)?;
        }
        let mut by_actor: Vec<&AudioTimelineClip> = self.clips.iter().collect();
        by_actor.sort_by(|a, b| {
            (&a.actor, a.start_sample, a.id).cmp(&(&b.actor, b.start_sample, b.id))
        });
        for pair in by_actor.windows(2) {
            if pair[0].actor == pair[1].actor && pair[1].start_sample < pair[0].end_sample() {
                return Err(AudioTimelineError::OverlappingActorClips {
                    actor: pair[0].actor.clone(),
                    first: pair[0].id,
                    second: pair[1].id,
                });
            }
        }
        Ok(())
    }

    /// Strict JSON decoding: unknown fields and other versions are errors.
    pub fn from_json(bytes: &[u8]) -> Result<Self, String> {
        let timeline: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        timeline.validate().map_err(|e| e.to_string())?;
        Ok(timeline)
    }

    pub fn to_json_pretty(&self) -> Result<String, String> {
        self.validate().map_err(|e| e.to_string())?;
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// The actor's clip playing at scene sample `sample` (at most one after validation).
    pub fn clip_at(&self, actor: &str, sample: u64) -> Option<&AudioTimelineClip> {
        self.clips
            .iter()
            .find(|c| c.actor == actor && c.source_sample_at(sample).is_some())
    }

    pub fn viseme_at(&self, actor: &str, sample: u64) -> Viseme {
        self.clip_at(actor, sample)
            .map_or(Viseme::Rest, |clip| clip.viseme_at(sample))
    }

    pub fn blend_at(
        &self,
        actor: &str,
        sample: u64,
        transition_samples: u64,
    ) -> Result<VisemeBlend, AudioTimelineError> {
        // One second is far beyond any mouth transition; reject likely unit mistakes.
        if transition_samples > u64::from(DIALOGUE_SAMPLE_RATE) {
            return Err(AudioTimelineError::InvalidTransition);
        }
        Ok(self
            .clip_at(actor, sample)
            .map_or(VisemeBlend::hold(Viseme::Rest), |clip| {
                clip.blend_at(sample, transition_samples)
            }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use Viseme::*;

    fn cue(start: u64, end: u64, viseme: Viseme) -> VisemeCue {
        VisemeCue {
            start_sample: start,
            end_sample: end,
            viseme,
        }
    }
    fn clip() -> AudioTimelineClip {
        AudioTimelineClip {
            id: 0,
            actor: "AINO".into(),
            language: "fi-fi".into(),
            source: AudioSourceInfo::Imported {
                path: "audio/hei.wav".into(),
                pcm_sha256: "a".repeat(64),
            },
            source_samples: 1000,
            start_sample: 100,
            trim_start_samples: 0,
            duration_samples: 1000,
            gain: 1.0,
            mouth: Some(MouthTiming {
                source: AlignmentSource::Manual,
                phonemes: vec![],
                // Gap [0,200) is Rest; [200,400) Closed; [400,800) A; [800,1000) Rest gap.
                visemes: vec![cue(200, 400, Closed), cue(400, 800, A)],
            }),
        }
    }
    fn timeline() -> AudioTimeline {
        let mut t = AudioTimeline::new("intro", 2000);
        t.clips.push(clip());
        t
    }

    #[test]
    fn placement_matches_mix_clip_and_mouth_follows_trim() {
        let mut t = timeline();
        t.validate().unwrap();
        assert_eq!(t.viseme_at("AINO", 99), Rest);
        assert_eq!(t.viseme_at("AINO", 300), Closed); // local 200
        assert_eq!(t.viseme_at("AINO", 500), A); // local 400: half-open boundary
        assert_eq!(t.viseme_at("AINO", 1100), Rest); // clip end is exclusive
        t.clips[0].trim_start_samples = 300;
        t.clips[0].duration_samples = 600;
        t.validate().unwrap();
        // Scene 100 now plays local 300 (still Closed); local 400 at scene 200.
        assert_eq!(t.viseme_at("AINO", 100), Closed);
        assert_eq!(t.viseme_at("AINO", 200), A);
        let audio = AudioBuffer {
            sample_rate: 24_000,
            channels: 1,
            samples: (0..1000).map(|i| i as f32 / 1000.0).collect(),
        };
        let mixed =
            motion_audio::mix_timeline(&[t.clips[0].mix_clip(audio).unwrap()], 24_000, 1, 2000)
                .unwrap();
        assert_eq!(mixed.samples[99], 0.0);
        assert_eq!(mixed.samples[100], 0.3); // source sample 300 at scene 100
        assert_eq!(mixed.samples[699], 0.899);
        assert_eq!(mixed.samples[700], 0.0);
    }

    #[test]
    fn json_round_trip_keeps_version_and_provenance() {
        let mut t = timeline();
        t.clips.push(AudioTimelineClip {
            id: 1,
            actor: "EMMA".into(),
            language: "en-us".into(),
            source: AudioSourceInfo::Synthesized {
                provider: "kokoro".into(),
                model_revision: "hexgrad/Kokoro-82M@rev".into(),
                voice: "af_heart".into(),
                voice_revision: None,
                cache_key: "k".into(),
                pcm_sha256: "b".repeat(64),
            },
            mouth: Some(MouthTiming {
                source: AlignmentSource::Estimated {
                    tool: "kilsat-english-estimator".into(),
                    version: "1".into(),
                },
                phonemes: vec![SamplePhoneme {
                    phoneme: "h".into(),
                    start_sample: 0,
                    end_sample: 10,
                }],
                visemes: vec![cue(0, 10, Rest)],
            }),
            ..clip()
        });
        let json = t.to_json_pretty().unwrap();
        assert!(json.contains("\"format_version\": 1"));
        assert!(json.contains("\"kind\": \"manual\""));
        assert!(json.contains("\"kind\": \"synthesized\""));
        let back = AudioTimeline::from_json(json.as_bytes()).unwrap();
        assert_eq!(back, t);
        // Strict decoding.
        let future = json.replacen("\"format_version\": 1", "\"format_version\": 2", 1);
        assert!(AudioTimeline::from_json(future.as_bytes())
            .unwrap_err()
            .contains("format_version 2"));
        let unknown = json.replacen("\"scene_id\"", "\"extra\": 1, \"scene_id\"", 1);
        assert!(AudioTimeline::from_json(unknown.as_bytes()).is_err());
    }

    #[test]
    fn invalid_timelines_are_typed_errors() {
        let check = |edit: fn(&mut AudioTimeline), expect: fn(&AudioTimelineError) -> bool| {
            let mut t = timeline();
            edit(&mut t);
            let error = t.validate().unwrap_err();
            assert!(expect(&error), "{error:?}");
        };
        use AudioTimelineError as E;
        check(
            |t| t.sample_rate = 48_000,
            |e| matches!(e, E::UnsupportedSampleRate(48_000)),
        );
        check(
            |t| t.clips[0].trim_start_samples = 1,
            |e| matches!(e, E::Placement { .. }),
        );
        check(
            |t| t.clips[0].start_sample = 1001,
            |e| matches!(e, E::Placement { .. }),
        );
        check(
            |t| t.clips[0].gain = f32::NAN,
            |e| matches!(e, E::InvalidGain(0)),
        );
        check(
            |t| t.clips[0].actor.clear(),
            |e| matches!(e, E::InvalidLabel("actor")),
        );
        check(
            |t| {
                t.clips[0].source = AudioSourceInfo::Imported {
                    path: "a.wav".into(),
                    pcm_sha256: "x".into(),
                }
            },
            |e| matches!(e, E::InvalidProvenance { .. }),
        );
        check(
            |t| t.clips[0].mouth.as_mut().unwrap().visemes[1].start_sample = 399,
            |e| matches!(e, E::MouthIntervals { .. }),
        );
        check(
            |t| t.clips[0].mouth.as_mut().unwrap().visemes[1].end_sample = 1001,
            |e| matches!(e, E::MouthIntervals { .. }),
        );
        check(
            |t| {
                t.clips[0].mouth.as_mut().unwrap().phonemes = vec![SamplePhoneme {
                    phoneme: "ʘ".into(),
                    start_sample: 0,
                    end_sample: 1,
                }]
            },
            |e| matches!(e, E::MouthIntervals { .. }),
        );
        check(
            |t| {
                let mut second = t.clips[0].clone();
                second.id = 1;
                second.start_sample = 1099;
                second.duration_samples = 1;
                t.clips.push(second);
            },
            |e| {
                matches!(
                    e,
                    E::OverlappingActorClips {
                        first: 0,
                        second: 1,
                        ..
                    }
                )
            },
        );
        check(
            |t| t.clips.push(t.clips[0].clone()),
            |e| matches!(e, E::DuplicateClipId(0)),
        );
        // Touching clips for one actor are fine; other actors may overlap.
        let mut t = timeline();
        let mut next = t.clips[0].clone();
        next.id = 1;
        next.start_sample = 1100;
        next.duration_samples = 900;
        t.clips.push(next);
        let mut other = t.clips[0].clone();
        other.id = 2;
        other.actor = "EMMA".into();
        t.clips.push(other);
        t.validate().unwrap();
        assert!(t.clips[0]
            .mix_clip(AudioBuffer {
                sample_rate: 24_000,
                channels: 1,
                samples: vec![0.0; 999]
            })
            .is_err());
    }

    #[test]
    fn blend_crossfades_symmetrically_at_boundaries() {
        let t = timeline();
        // Boundary Closed→A at local 400 = scene 500. Closed lasts 200, A 400, so
        // h = min(100/2, 200/2, 400/2) = 50 with a 100-sample transition.
        let b = |s| t.blend_at("AINO", s, 100).unwrap();
        assert_eq!(b(449), VisemeBlend::hold(Closed));
        assert_eq!(
            b(450),
            VisemeBlend {
                from: Closed,
                to: A,
                weight: 0.0
            }
        );
        assert_eq!(
            b(475),
            VisemeBlend {
                from: Closed,
                to: A,
                weight: 0.25
            }
        );
        assert_eq!(
            b(500),
            VisemeBlend {
                from: Closed,
                to: A,
                weight: 0.5
            }
        );
        assert_eq!(b(500).dominant(), A);
        assert_eq!(
            b(549),
            VisemeBlend {
                from: Closed,
                to: A,
                weight: 0.99
            }
        );
        assert_eq!(b(550), VisemeBlend::hold(A));
        // Rest gap → Closed at local 200 (scene 300).
        assert_eq!(
            b(250),
            VisemeBlend {
                from: Rest,
                to: Closed,
                weight: 0.0
            }
        );
        assert_eq!(
            b(300),
            VisemeBlend {
                from: Rest,
                to: Closed,
                weight: 0.5
            }
        );
        // Weights rise monotonically across a window.
        let weights: Vec<f32> = (450..550).map(|s| b(s).weight).collect();
        assert!(weights.windows(2).all(|w| w[0] < w[1]));
        // A hard cut and the outside of the clip.
        assert_eq!(t.blend_at("AINO", 500, 0).unwrap(), VisemeBlend::hold(A));
        assert_eq!(
            t.blend_at("AINO", 50, 100).unwrap(),
            VisemeBlend::hold(Rest)
        );
        assert_eq!(
            t.blend_at("NOBODY", 500, 100).unwrap(),
            VisemeBlend::hold(Rest)
        );
        assert!(t.blend_at("AINO", 0, 24_001).is_err());
        // Long transitions are clamped to half of the shorter neighbour.
        let wide = |s| t.blend_at("AINO", s, 10_000).unwrap();
        assert_eq!(
            wide(399),
            VisemeBlend {
                from: Rest,
                to: Closed,
                weight: 0.995
            }
        );
        assert_eq!(
            wide(400),
            VisemeBlend {
                from: Closed,
                to: A,
                weight: 0.0
            }
        );
        assert_eq!(
            wide(500),
            VisemeBlend {
                from: Closed,
                to: A,
                weight: 0.5
            }
        );
    }

    #[test]
    fn blend_treats_trimmed_edges_and_equal_neighbours_correctly() {
        let mut t = timeline();
        // Equal adjacent cues merge into one segment: no blend at local 300.
        t.clips[0].mouth.as_mut().unwrap().visemes = vec![
            cue(200, 300, Closed),
            cue(300, 400, Closed),
            cue(400, 800, A),
        ];
        assert_eq!(
            t.blend_at("AINO", 400, 100).unwrap(),
            VisemeBlend::hold(Closed)
        );
        assert_eq!(t.blend_at("AINO", 500, 100).unwrap().weight, 0.5);
        // Trim into the A segment: the clip starts on A with a hard cut, like its audio.
        t.clips[0].trim_start_samples = 600;
        t.clips[0].duration_samples = 400;
        // Visible window local [600,1000): A [600,800) then Rest gap [800,1000).
        assert_eq!(t.blend_at("AINO", 100, 100).unwrap(), VisemeBlend::hold(A));
        assert_eq!(
            t.blend_at("AINO", 499, 100).unwrap(),
            VisemeBlend::hold(Rest)
        ); // last sample
        assert_eq!(
            t.blend_at("AINO", 300, 100).unwrap(),
            VisemeBlend {
                from: A,
                to: Rest,
                weight: 0.5
            }
        );
        // Seek determinism: the same sample gives the same blend however it is reached.
        let forward: Vec<_> = (100..500)
            .map(|s| t.blend_at("AINO", s, 100).unwrap())
            .collect();
        let backward: Vec<_> = (100..500)
            .rev()
            .map(|s| t.blend_at("AINO", s, 100).unwrap())
            .collect();
        assert!(forward.iter().eq(backward.iter().rev()));
    }
}
