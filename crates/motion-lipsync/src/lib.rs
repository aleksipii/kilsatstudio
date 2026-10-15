//! Explicit alignment provenance: synthesis tokens are not phoneme timestamps.

use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Viseme {
    Rest,
    Closed,
    TeethLip,
    TongueTeeth,
    TongueUp,
    Sibilant,
    Sh,
    R,
    A,
    E,
    I,
    O,
    U,
}

/// Small English IPA inventory. Unknown phonemes fail instead of inventing mouth shapes.
pub fn phoneme_to_viseme(phoneme: &str) -> Option<Viseme> {
    let normalized = phoneme.trim().replace(['ˈ', 'ˌ', 'ː', 'ˑ'], "");
    Some(match normalized.as_str() {
        "" | "sil" | "sp" | "pau" => Viseme::Rest,
        "p" | "b" | "m" => Viseme::Closed,
        "f" | "v" => Viseme::TeethLip,
        "θ" | "ð" => Viseme::TongueTeeth,
        "t" | "d" | "n" | "l" | "ɾ" => Viseme::TongueUp,
        "s" | "z" => Viseme::Sibilant,
        "ʃ" | "ʒ" | "tʃ" | "dʒ" | "t͡ʃ" | "d͡ʒ" => Viseme::Sh,
        "r" | "ɹ" | "ɚ" | "ɝ" => Viseme::R,
        "ɑ" | "ɒ" | "a" | "æ" | "ʌ" | "aɪ" | "aʊ" => Viseme::A,
        "ɛ" | "e" | "ə" | "ɜ" | "eɪ" => Viseme::E,
        "i" | "ɪ" | "j" => Viseme::I,
        "o" | "ɔ" | "oʊ" | "əʊ" | "ɔɪ" => Viseme::O,
        "u" | "ʊ" | "w" => Viseme::U,
        "k" | "ɡ" | "g" | "ŋ" | "h" | "ʔ" => Viseme::Rest,
        _ => return None,
    })
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TimedPhoneme {
    pub phoneme: String,
    pub start_seconds: f64,
    pub end_seconds: f64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum AlignmentSource {
    /// Human authored or corrected against the waveform.
    Manual,
    /// External aligner output, not automatically a quality certification.
    Forced { tool: String, version: String },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Alignment {
    pub source: AlignmentSource,
    pub duration_seconds: f64,
    pub phonemes: Vec<TimedPhoneme>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TimedViseme {
    pub viseme: Viseme,
    pub start_seconds: f64,
    pub end_seconds: f64,
}
#[derive(Debug, PartialEq)]
pub struct AlignmentError(pub String);
impl fmt::Display for AlignmentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for AlignmentError {}

impl Alignment {
    /// Structural validation only. Perceptual quality must be reviewed with real audio.
    pub fn validate(&self) -> Result<(), AlignmentError> {
        if !self.duration_seconds.is_finite() || self.duration_seconds < 0.0 {
            return Err(AlignmentError("invalid alignment duration".into()));
        }
        if let AlignmentSource::Forced { tool, version } = &self.source {
            if tool.trim().is_empty() || version.trim().is_empty() {
                return Err(AlignmentError(
                    "forced alignment requires tool and version".into(),
                ));
            }
        }
        let mut previous_end = 0.0;
        for phoneme in &self.phonemes {
            if !phoneme.start_seconds.is_finite()
                || !phoneme.end_seconds.is_finite()
                || phoneme.start_seconds < previous_end
                || phoneme.end_seconds <= phoneme.start_seconds
                || phoneme.end_seconds > self.duration_seconds
            {
                return Err(AlignmentError("phoneme intervals must be finite, ordered, non-overlapping and within audio duration".into()));
            }
            if phoneme_to_viseme(&phoneme.phoneme).is_none() {
                return Err(AlignmentError(format!(
                    "unsupported IPA phoneme: {}",
                    phoneme.phoneme
                )));
            }
            previous_end = phoneme.end_seconds;
        }
        Ok(())
    }
    pub fn visemes(&self) -> Result<Vec<TimedViseme>, AlignmentError> {
        self.validate()?;
        Ok(self
            .phonemes
            .iter()
            .map(|p| TimedViseme {
                viseme: phoneme_to_viseme(&p.phoneme).expect("validated phoneme"),
                start_seconds: p.start_seconds,
                end_seconds: p.end_seconds,
            })
            .collect())
    }
    /// Random access supports seeking; gaps and positions outside audio close the mouth.
    pub fn viseme_at(&self, local_seconds: f64) -> Result<Viseme, AlignmentError> {
        self.validate()?;
        if !local_seconds.is_finite() {
            return Err(AlignmentError("invalid playback position".into()));
        }
        Ok(self
            .phonemes
            .iter()
            .find(|p| p.start_seconds <= local_seconds && local_seconds < p.end_seconds)
            .and_then(|p| phoneme_to_viseme(&p.phoneme))
            .unwrap_or(Viseme::Rest))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Alignment {
        Alignment {
            source: AlignmentSource::Manual,
            duration_seconds: 1.0,
            phonemes: vec![
                TimedPhoneme {
                    phoneme: "m".into(),
                    start_seconds: 0.1,
                    end_seconds: 0.3,
                },
                TimedPhoneme {
                    phoneme: "ɑː".into(),
                    start_seconds: 0.3,
                    end_seconds: 0.8,
                },
            ],
        }
    }
    #[test]
    fn ipa_mapping_and_half_open_boundaries() {
        let a = fixture();
        assert_eq!(a.viseme_at(0.0).unwrap(), Viseme::Rest);
        assert_eq!(a.viseme_at(0.1).unwrap(), Viseme::Closed);
        assert_eq!(a.viseme_at(0.3).unwrap(), Viseme::A);
        assert_eq!(a.viseme_at(0.8).unwrap(), Viseme::Rest);
        assert_eq!(phoneme_to_viseme("t͡ʃ"), Some(Viseme::Sh));
    }
    #[test]
    fn rejects_overlaps_and_unmapped_phonemes() {
        let mut a = fixture();
        a.phonemes[1].start_seconds = 0.2;
        assert!(a.validate().is_err());
        a.phonemes[1].start_seconds = 0.3;
        a.phonemes[1].phoneme = "unknown".into();
        assert!(a.visemes().is_err());
    }
    #[test]
    fn rejects_nonfinite_and_out_of_bounds() {
        let mut a = fixture();
        a.phonemes[0].end_seconds = f64::NAN;
        assert!(a.validate().is_err());
        let mut a = fixture();
        a.phonemes[1].end_seconds = 2.0;
        assert!(a.validate().is_err());
    }
}
