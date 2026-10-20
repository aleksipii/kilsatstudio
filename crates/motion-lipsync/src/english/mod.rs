//! Open-vocabulary English text to estimated mouth-shape data.
//!
//! Pipeline: [`normalize`] written text to spoken words, pronounce each word
//! (override, then CMUdict, then derived inflection, then rule-based fallback), then
//! [`estimate_viseme_track`] turns phonemes into a timed viseme track. Everything is
//! deterministic and offline. The track is always labelled `Estimated`; nothing here
//! aligns to audio, and its perceptual quality has not been measured.
mod g2p;
mod lexicon;
mod normalize;
mod visemes;

pub use g2p::letter_to_sound;
pub use lexicon::Lexicon;
pub use normalize::{cardinal_words, normalize, spoken_words, Pause, Token, MAX_TOKENS};
pub use visemes::{
    estimate_viseme_track, phoneme_viseme, EstimatedVisemeTrack, VisemeCue, DIALOGUE_SAMPLE_RATE,
    MAX_TRACK_FRAMES, TOOL_NAME, TOOL_VERSION,
};

use serde::Serialize;
use std::collections::BTreeMap;
use std::fmt;

/// The 39 ARPAbet phonemes used by CMUdict (stress digits are not represented).
pub const ARPABET: [&str; 39] = [
    "AA", "AE", "AH", "AO", "AW", "AY", "B", "CH", "D", "DH", "EH", "ER", "EY", "F", "G", "HH",
    "IH", "IY", "JH", "K", "L", "M", "N", "NG", "OW", "OY", "P", "R", "S", "SH", "T", "TH", "UH",
    "UW", "V", "W", "Y", "Z", "ZH",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Phoneme(u8);

impl Phoneme {
    /// Parses an ARPAbet symbol, case-insensitive, with an optional stress digit 0..=2.
    pub fn parse(s: &str) -> Option<Phoneme> {
        let up = s.trim().to_ascii_uppercase();
        let base = up.strip_suffix(['0', '1', '2']).unwrap_or(&up);
        ARPABET
            .iter()
            .position(|a| *a == base)
            .map(|i| Phoneme(i as u8))
    }

    pub fn symbol(self) -> &'static str {
        ARPABET[self.0 as usize]
    }

    pub fn is_vowel(self) -> bool {
        matches!(
            self.symbol(),
            "AA" | "AE"
                | "AH"
                | "AO"
                | "AW"
                | "AY"
                | "EH"
                | "ER"
                | "EY"
                | "IH"
                | "IY"
                | "OW"
                | "OY"
                | "UH"
                | "UW"
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnglishError {
    /// Text contained nothing speakable after normalisation.
    NoSpeakableText,
    TooLong(String),
    InvalidFrameCount(u64),
    Pronunciation(String),
}

impl fmt::Display for EnglishError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoSpeakableText => f.write_str("text has no speakable words"),
            Self::TooLong(m) | Self::Pronunciation(m) => f.write_str(m),
            Self::InvalidFrameCount(n) => write!(f, "invalid audio frame count {n}"),
        }
    }
}
impl std::error::Error for EnglishError {}

pub const MAX_OVERRIDES: usize = 1024;
pub const MAX_OVERRIDE_WORD: usize = 64;
pub const MAX_OVERRIDE_PHONEMES: usize = 32;

/// Validated per-project pronunciation overrides (word to phonemes).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Pronunciations(BTreeMap<String, Vec<Phoneme>>);

impl Pronunciations {
    pub fn new() -> Self {
        Self::default()
    }

    /// Validates a MotionScript `pronunciations:` map. Keys are lowercase words
    /// (`a-z` and inner `'`); values are space-separated ARPAbet phonemes (optional
    /// stress digits are accepted and ignored). At most [`MAX_OVERRIDES`] entries.
    pub fn from_map(map: &BTreeMap<String, String>) -> Result<Self, EnglishError> {
        let bad = |m: String| EnglishError::Pronunciation(m);
        if map.len() > MAX_OVERRIDES {
            return Err(bad(format!(
                "pronunciations has {} entries; at most {MAX_OVERRIDES} are allowed",
                map.len()
            )));
        }
        let mut out = BTreeMap::new();
        for (word, phones) in map {
            let ok_word = !word.is_empty()
                && word.len() <= MAX_OVERRIDE_WORD
                && word.bytes().all(|b| b.is_ascii_lowercase() || b == b'\'')
                && word.bytes().next().is_some_and(|b| b != b'\'')
                && !word.ends_with('\'');
            if !ok_word {
                return Err(bad(format!(
                    "pronunciation word {word:?} must be 1..={MAX_OVERRIDE_WORD} lowercase letters or inner apostrophes"
                )));
            }
            let parsed: Option<Vec<Phoneme>> =
                phones.split_whitespace().map(Phoneme::parse).collect();
            match parsed {
                Some(v) if !v.is_empty() && v.len() <= MAX_OVERRIDE_PHONEMES => {
                    out.insert(word.clone(), v);
                }
                _ => {
                    return Err(bad(format!(
                        "pronunciation for {word:?} must be 1..={MAX_OVERRIDE_PHONEMES} ARPAbet phonemes (e.g. \"L UH K S M AE K S IH NG\"), got {phones:?}"
                    )))
                }
            }
        }
        Ok(Self(out))
    }

    pub fn get(&self, word: &str) -> Option<&[Phoneme]> {
        self.0.get(word).map(Vec::as_slice)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PronunciationSource {
    /// Project override from `pronunciations:`.
    Override,
    /// Exact CMUdict entry (or built-in letter-name supplement).
    Lexicon,
    /// CMUdict base word plus a regular `'s` / plural suffix.
    Derived,
    /// Rule-based letter-to-sound guess.
    Fallback,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct WordPronunciation {
    pub word: String,
    pub phonemes: Vec<&'static str>,
    pub source: PronunciationSource,
    /// True for fallback words: the pronunciation is a guess; add an override.
    pub low_confidence: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PronunciationReport {
    pub words: Vec<WordPronunciation>,
}

impl PronunciationReport {
    pub fn low_confidence_words(&self) -> Vec<&str> {
        let mut v: Vec<&str> = self
            .words
            .iter()
            .filter(|w| w.low_confidence)
            .map(|w| w.word.as_str())
            .collect();
        v.sort_unstable();
        v.dedup();
        v
    }

    pub fn count(&self, source: PronunciationSource) -> usize {
        self.words.iter().filter(|w| w.source == source).count()
    }
}

/// Pronounce one normalised word. Order: override, lexicon, derived suffix, fallback.
pub fn pronounce_word(
    word: &str,
    overrides: &Pronunciations,
    lexicon: &Lexicon,
) -> (Vec<Phoneme>, PronunciationSource) {
    if let Some(p) = overrides.get(word) {
        return (p.to_vec(), PronunciationSource::Override);
    }
    if let Some(p) = lexicon.lookup(word) {
        return (p, PronunciationSource::Lexicon);
    }
    for (suffix, plural) in [("'s", true), ("s'", true), ("s", true)] {
        if let Some(base) = word.strip_suffix(suffix) {
            let base = if suffix == "s'" {
                // "dogs'" is the plural "dogs".
                &word[..word.len() - 1]
            } else {
                base
            };
            if base.len() >= 2 && plural {
                if let Some(mut p) = overrides
                    .get(base)
                    .map(<[_]>::to_vec)
                    .or_else(|| lexicon.lookup(base))
                {
                    let last = p.last().map(|l| l.symbol()).unwrap_or("");
                    let ending = match last {
                        "S" | "Z" | "SH" | "ZH" | "CH" | "JH" => vec!["IH", "Z"],
                        "P" | "T" | "K" | "F" | "TH" => vec!["S"],
                        _ => vec!["Z"],
                    };
                    p.extend(ending.into_iter().filter_map(Phoneme::parse));
                    return (p, PronunciationSource::Derived);
                }
            }
        }
    }
    (letter_to_sound(word), PronunciationSource::Fallback)
}

/// Pre-render report: every spoken word with its pronunciation source.
pub fn pronunciation_report(text: &str, overrides: &Pronunciations) -> PronunciationReport {
    let lex = Lexicon::builtin();
    let words = spoken_words(text, lex)
        .into_iter()
        .map(|word| {
            let (p, source) = pronounce_word(&word, overrides, lex);
            WordPronunciation {
                phonemes: p.iter().map(|x| x.symbol()).collect(),
                low_confidence: source == PronunciationSource::Fallback,
                source,
                word,
            }
        })
        .collect();
    PronunciationReport { words }
}

#[cfg(test)]
mod tests;
