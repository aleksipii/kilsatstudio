//! Phoneme to viseme mapping and the deterministic estimated-duration model.
use super::{
    normalize, pronounce_word, EnglishError, Lexicon, Pause, Phoneme, PronunciationSource,
    Pronunciations, Token,
};
use crate::{AlignmentSource, Viseme};
use serde::{Deserialize, Serialize};

/// The dialogue sample clock shared by speech, mouth evaluation and export.
pub const DIALOGUE_SAMPLE_RATE: u32 = motion_audio::clock::DIALOGUE_SAMPLE_RATE;
/// Same 300 s bound as `EstimatedVisemeAlignment`.
pub const MAX_TRACK_FRAMES: u64 = 24_000 * 300;
pub const TOOL_NAME: &str = "kilsat-english-estimator";
pub const TOOL_VERSION: &str = "1";

const SHORT_PAUSE_MS: u64 = 120;
const LONG_PAUSE_MS: u64 = 300;

/// Half-open `[start_sample, end_sample)` interval at 24 kHz showing one viseme.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VisemeCue {
    pub start_sample: u64,
    pub end_sample: u64,
    pub viseme: Viseme,
}

/// Estimated mouth track. Cues are contiguous from sample 0 to `frame_count`, with
/// pauses shown as `Viseme::Rest`. `source` is always `AlignmentSource::Estimated`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EstimatedVisemeTrack {
    pub source: AlignmentSource,
    pub sample_rate: u32,
    pub frame_count: u64,
    pub cues: Vec<VisemeCue>,
    /// Words pronounced by the rule-based fallback (low confidence).
    pub fallback_words: usize,
    /// Audio duration divided by the unscaled model duration; 1.0 when no audio
    /// length was supplied. Values far from 1.0 mean the model rate is a poor fit.
    pub time_scale: f64,
}

impl EstimatedVisemeTrack {
    /// Viseme at a sample (half-open intervals; outside the track is `Rest`).
    pub fn viseme_at_sample(&self, sample: u64) -> Viseme {
        let i = self.cues.partition_point(|c| c.end_sample <= sample);
        self.cues
            .get(i)
            .filter(|c| c.start_sample <= sample)
            .map_or(Viseme::Rest, |c| c.viseme)
    }

    /// Lossy conversion to the Rhubarb-style cues used by `EstimatedVisemeAlignment`,
    /// through the shared projection `mouth_chart::MouthChart::from_viseme`.
    /// Rhubarb's nine shapes cannot express Sibilant, Sh, R or TongueTeeth, which
    /// collapse to the nearest shape (C, C, C, H); prefer the viseme cues.
    pub fn to_mouth_cues(&self) -> Vec<crate::rhubarb::MouthCue> {
        self.cues
            .iter()
            .map(|c| crate::rhubarb::MouthCue {
                start_sample: c.start_sample,
                end_sample: c.end_sample,
                shape: crate::mouth_chart::MouthChart::from_viseme(c.viseme).letter(),
            })
            .collect()
    }
}

/// Mapping documented in docs/english-lipsync.md. `None` for velars and /h/, which
/// have no visible shape of their own and borrow a neighbouring vowel (see below).
pub fn phoneme_viseme(p: Phoneme) -> Option<Viseme> {
    Some(match p.symbol() {
        "P" | "B" | "M" => Viseme::Closed,
        "F" | "V" => Viseme::TeethLip,
        "TH" | "DH" => Viseme::TongueTeeth,
        "T" | "D" | "N" | "L" => Viseme::TongueUp,
        "S" | "Z" => Viseme::Sibilant,
        "SH" | "ZH" | "CH" | "JH" => Viseme::Sh,
        "R" | "ER" => Viseme::R,
        "AA" | "AE" | "AH" | "AW" | "AY" => Viseme::A,
        "EH" | "EY" => Viseme::E,
        "IH" | "IY" | "Y" => Viseme::I,
        "AO" | "OW" | "OY" => Viseme::O,
        "UH" | "UW" | "W" => Viseme::U,
        _ => return None, // K G NG HH
    })
}

fn phoneme_ms(p: Phoneme) -> u64 {
    match p.symbol() {
        "AA" | "AE" | "AO" | "ER" | "UW" => 110,
        "AH" | "IH" => 70,
        "AW" | "AY" | "OY" => 140,
        "EY" | "OW" => 130,
        "EH" => 90,
        "IY" => 100,
        "UH" => 80,
        "CH" | "JH" => 100,
        "F" | "V" | "S" | "Z" | "SH" | "ZH" | "TH" | "DH" => 80,
        "L" | "R" | "W" | "Y" | "M" | "N" | "NG" => 70,
        _ => 60, // stops and /h/
    }
}

struct Segment {
    viseme: Viseme,
    ms: u64,
}

/// Estimates a viseme track for `text`. With `audio_frames` (24 kHz sample count)
/// the model timing is scaled so the cues cover exactly the audio; otherwise the
/// unscaled model duration is used. Result is deterministic for identical inputs.
pub fn estimate_viseme_track(
    text: &str,
    overrides: &Pronunciations,
    audio_frames: Option<u64>,
) -> Result<EstimatedVisemeTrack, EnglishError> {
    if let Some(n) = audio_frames {
        if n == 0 || n > MAX_TRACK_FRAMES {
            return Err(EnglishError::InvalidFrameCount(n));
        }
    }
    let lex = Lexicon::builtin();
    let tokens = normalize(text, lex);
    let mut segments: Vec<Segment> = Vec::new();
    let mut fallback_words = 0;
    let mut spoke = false;
    for token in &tokens {
        match token {
            Token::Pause(p) => segments.push(Segment {
                viseme: Viseme::Rest,
                ms: match p {
                    Pause::Short => SHORT_PAUSE_MS,
                    Pause::Long => LONG_PAUSE_MS,
                },
            }),
            Token::Word(w) => {
                spoke = true;
                let (phones, source) = pronounce_word(w, overrides, lex);
                if source == PronunciationSource::Fallback {
                    fallback_words += 1;
                }
                let base: Vec<Option<Viseme>> = phones.iter().map(|p| phoneme_viseme(*p)).collect();
                for (i, p) in phones.iter().enumerate() {
                    let viseme = base[i].unwrap_or_else(|| {
                        // Velars and /h/ borrow the next shaped phoneme in the word,
                        // else the previous one, else rest.
                        base[i + 1..]
                            .iter()
                            .flatten()
                            .next()
                            .or_else(|| base[..i].iter().rev().flatten().next())
                            .copied()
                            .unwrap_or(Viseme::Rest)
                    });
                    segments.push(Segment {
                        viseme,
                        ms: phoneme_ms(*p),
                    });
                }
            }
        }
    }
    if !spoke || segments.is_empty() {
        return Err(EnglishError::NoSpeakableText);
    }
    let model_ms: u64 = segments.iter().map(|s| s.ms).sum();
    let frames = match audio_frames {
        Some(n) => n,
        None => model_ms * (DIALOGUE_SAMPLE_RATE as u64 / 1000),
    };
    if frames > MAX_TRACK_FRAMES {
        return Err(EnglishError::TooLong(format!(
            "estimated speech is {frames} samples; the limit is {MAX_TRACK_FRAMES}"
        )));
    }
    let mut cues: Vec<VisemeCue> = Vec::with_capacity(segments.len());
    let mut cumulative = 0u64;
    let mut start = 0u64;
    for s in &segments {
        cumulative += s.ms;
        let end = (cumulative as u128 * frames as u128 / model_ms as u128) as u64;
        if end <= start {
            continue;
        }
        match cues.last_mut() {
            Some(last) if last.viseme == s.viseme => last.end_sample = end,
            _ => cues.push(VisemeCue {
                start_sample: start,
                end_sample: end,
                viseme: s.viseme,
            }),
        }
        start = end;
    }
    if cues.is_empty() {
        // Fewer samples than segments can only collapse to one cue, never to none.
        cues.push(VisemeCue {
            start_sample: 0,
            end_sample: frames,
            viseme: Viseme::Rest,
        });
    }
    Ok(EstimatedVisemeTrack {
        source: AlignmentSource::Estimated {
            tool: TOOL_NAME.into(),
            version: TOOL_VERSION.into(),
        },
        sample_rate: DIALOGUE_SAMPLE_RATE,
        frame_count: frames,
        cues,
        fallback_words,
        time_scale: frames as f64 / (model_ms * 24) as f64,
    })
}

/// As [`estimate_viseme_track`] with `Some(audio_frames)`, but the scaled estimate fills
/// only `[speech_start, speech_end)` and the mouth rests before and after it. Callers
/// pass the detected leading/trailing silence of the clip (see
/// `timing_eval::Envelope::speech_span`) so the mouth does not move over silence.
/// `time_scale` is relative to the speech span. Still `Estimated`; deterministic.
pub fn estimate_viseme_track_in_span(
    text: &str,
    overrides: &Pronunciations,
    audio_frames: u64,
    speech_start: u64,
    speech_end: u64,
) -> Result<EstimatedVisemeTrack, EnglishError> {
    if audio_frames == 0 || audio_frames > MAX_TRACK_FRAMES {
        return Err(EnglishError::InvalidFrameCount(audio_frames));
    }
    if speech_start >= speech_end || speech_end > audio_frames {
        return Err(EnglishError::InvalidFrameCount(
            speech_end.saturating_sub(speech_start),
        ));
    }
    let mut inner = estimate_viseme_track(text, overrides, Some(speech_end - speech_start))?;
    let mut cues = Vec::with_capacity(inner.cues.len() + 2);
    if speech_start > 0 {
        cues.push(VisemeCue {
            start_sample: 0,
            end_sample: speech_start,
            viseme: Viseme::Rest,
        });
    }
    for c in &inner.cues {
        let shifted = VisemeCue {
            start_sample: c.start_sample + speech_start,
            end_sample: c.end_sample + speech_start,
            viseme: c.viseme,
        };
        match cues.last_mut() {
            Some(last) if last.viseme == shifted.viseme => last.end_sample = shifted.end_sample,
            _ => cues.push(shifted),
        }
    }
    if speech_end < audio_frames {
        match cues.last_mut() {
            Some(last) if last.viseme == Viseme::Rest => last.end_sample = audio_frames,
            _ => cues.push(VisemeCue {
                start_sample: speech_end,
                end_sample: audio_frames,
                viseme: Viseme::Rest,
            }),
        }
    }
    inner.cues = cues;
    inner.frame_count = audio_frames;
    Ok(inner)
}
