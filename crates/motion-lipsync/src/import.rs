//! Import externally measured timings without running or downloading an aligner.
use crate::{Alignment, AlignmentError, AlignmentSource, TimedPhoneme};
use serde::Deserialize;

/// Identity of the exact audio submitted to the aligner. Use a content hash as id.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AlignmentAudio {
    pub asset_id: String,
    pub sample_rate: u32,
    pub frames: u64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportDocument {
    schema_version: u32,
    audio: AlignmentAudio,
    tool: String,
    tool_version: String,
    alphabet: Alphabet,
    phonemes: Vec<ImportedPhoneme>,
}
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum Alphabet {
    Ipa,
    Arpabet,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImportedPhoneme {
    phoneme: String,
    start_seconds: f64,
    end_seconds: f64,
}

/// Strict JSON import tied to the caller's actual audio metadata. Times are never
/// stretched or estimated. Passing structural checks does not certify accuracy.
pub fn import_alignment_json(
    json: &str,
    expected: &AlignmentAudio,
) -> Result<Alignment, AlignmentError> {
    let document: ImportDocument = serde_json::from_str(json)
        .map_err(|e| AlignmentError(format!("invalid alignment JSON: {e}")))?;
    if document.schema_version != 1 {
        return Err(AlignmentError(
            "unsupported alignment schema_version; expected 1".into(),
        ));
    }
    if expected.asset_id.trim().is_empty() || expected.sample_rate == 0 || expected.frames == 0 {
        return Err(AlignmentError(
            "audio requires a nonempty asset id, sample rate and frame count".into(),
        ));
    }
    if document.audio != *expected {
        return Err(AlignmentError("alignment audio identity or sample metadata mismatch; align the exact audio asset again".into()));
    }
    if document.phonemes.is_empty() {
        return Err(AlignmentError(
            "alignment contains no phonemes; export a phone tier rather than a word tier".into(),
        ));
    }
    let phonemes = document.phonemes.into_iter().enumerate().map(|(index, p)| {
        let phoneme = match document.alphabet {
            Alphabet::Ipa => p.phoneme,
            Alphabet::Arpabet => arpabet_to_ipa(&p.phoneme).ok_or_else(|| AlignmentError(format!("unsupported ARPAbet phone at interval {}: {}; export supported phone labels", index + 1, p.phoneme)))?.to_owned(),
        };
        Ok(TimedPhoneme { phoneme, start_seconds: p.start_seconds, end_seconds: p.end_seconds })
    }).collect::<Result<Vec<_>, AlignmentError>>()?;
    let alignment = Alignment {
        source: AlignmentSource::Forced {
            tool: document.tool,
            version: document.tool_version,
        },
        duration_seconds: expected.frames as f64 / expected.sample_rate as f64,
        phonemes,
    };
    alignment.validate()?;
    Ok(alignment)
}

/// Explicit English ARPAbet inventory. Only vowel stress suffixes 0/1/2 are
/// accepted; unknown/allophone labels fail so a converter can handle them explicitly.
pub fn arpabet_to_ipa(phone: &str) -> Option<&'static str> {
    let normalized = phone.trim();
    let base = normalized
        .strip_suffix(['0', '1', '2'])
        .unwrap_or(normalized);
    let vowel = match base {
        "AA" => Some("ɑ"),
        "AE" => Some("æ"),
        "AH" => Some(if normalized == "AH0" { "ə" } else { "ʌ" }),
        "AO" => Some("ɔ"),
        "AW" => Some("aʊ"),
        "AY" => Some("aɪ"),
        "EH" => Some("ɛ"),
        "ER" => Some(if normalized == "ER0" { "ɚ" } else { "ɝ" }),
        "EY" => Some("eɪ"),
        "IH" => Some("ɪ"),
        "IY" => Some("i"),
        "OW" => Some("oʊ"),
        "OY" => Some("ɔɪ"),
        "UH" => Some("ʊ"),
        "UW" => Some("u"),
        _ => None,
    };
    if vowel.is_some() {
        return vowel;
    }
    if base != normalized {
        return None;
    }
    Some(match base {
        "B" => "b",
        "CH" => "tʃ",
        "D" => "d",
        "DH" => "ð",
        "F" => "f",
        "G" => "ɡ",
        "HH" => "h",
        "JH" => "dʒ",
        "K" => "k",
        "L" => "l",
        "M" => "m",
        "N" => "n",
        "NG" => "ŋ",
        "P" => "p",
        "R" => "ɹ",
        "S" => "s",
        "SH" => "ʃ",
        "T" => "t",
        "TH" => "θ",
        "V" => "v",
        "W" => "w",
        "Y" => "j",
        "Z" => "z",
        "ZH" => "ʒ",
        "sil" | "sp" | "" => "sil",
        _ => return None,
    })
}
