//! Finnish orthographic mouth estimate. No English lexicon or acoustic alignment.
use crate::{
    english::{EstimatedVisemeTrack, VisemeCue, MAX_TRACK_FRAMES},
    AlignmentError, AlignmentSource, Viseme,
};
use sha2::{Digest, Sha256};
pub const TOOL: &str = "kilsat-finnish-orthographic-estimator";
pub const VERSION: &str = "1";
pub fn identity(text: &str, frames: u64, start: u64, end: u64) -> String {
    let mut hash = Sha256::new();
    for value in [TOOL, VERSION, "fi-fi", text] {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    for value in [frames, start, end] {
        hash.update(value.to_le_bytes());
    }
    format!("{:x}", hash.finalize())
}
fn source() -> AlignmentSource {
    AlignmentSource::Estimated {
        tool: TOOL.into(),
        version: VERSION.into(),
    }
}
fn push(cues: &mut Vec<VisemeCue>, start: u64, end: u64, viseme: Viseme) {
    if start >= end {
        return;
    }
    if let Some(last) = cues
        .last_mut()
        .filter(|c| c.viseme == viseme && c.end_sample == start)
    {
        last.end_sample = end;
    } else {
        cues.push(VisemeCue {
            start_sample: start,
            end_sample: end,
            viseme,
        });
    }
}
pub fn silent_track(frames: u64) -> Result<EstimatedVisemeTrack, AlignmentError> {
    if frames == 0 || frames > MAX_TRACK_FRAMES {
        return Err(AlignmentError(
            "Finnish estimate audio duration exceeds bounds".into(),
        ));
    }
    Ok(EstimatedVisemeTrack {
        source: source(),
        sample_rate: 24_000,
        frame_count: frames,
        cues: vec![VisemeCue {
            start_sample: 0,
            end_sample: frames,
            viseme: Viseme::Rest,
        }],
        fallback_words: 0,
        time_scale: 1.0,
    })
}
/// Coarse visible-letter weights fill only the measured non-silent speech span.
/// Unsupported text needs reviewed timings; no English pronunciation fallback.
pub fn estimate(
    text: &str,
    frames: u64,
    start: u64,
    end: u64,
) -> Result<EstimatedVisemeTrack, AlignmentError> {
    if frames == 0 || frames > MAX_TRACK_FRAMES || start >= end || end > frames {
        return Err(AlignmentError("invalid Finnish speech span".into()));
    }
    if text.trim().is_empty() || text.len() > 16_000 || text.chars().count() > 4096 {
        return Err(AlignmentError(
            "Finnish estimate needs 1..4096 letters within 16KB".into(),
        ));
    }
    let mut units = Vec::new();
    let mut letters = 0usize;
    for c in text.chars().flat_map(char::to_lowercase) {
        let (shape, ms) = match c {
            'a' | 'ä' => (Viseme::A, 100u64),
            'e' => (Viseme::E, 95),
            'i' | 'j' => (Viseme::I, 90),
            'o' | 'ö' | 'å' => (Viseme::O, 100),
            'u' | 'y' => (Viseme::U, 100),
            'p' | 'b' | 'm' => (Viseme::Closed, 65),
            'f' | 'v' | 'w' => (Viseme::TeethLip, 65),
            's' | 'z' => (Viseme::Sibilant, 70),
            't' | 'd' | 'n' | 'l' => (Viseme::TongueUp, 60),
            'r' => (Viseme::R, 65),
            'k' | 'g' | 'h' | 'q' | 'c' | 'x' => (Viseme::Rest, 55),
            c if c.is_whitespace() => (Viseme::Rest, 45),
            ',' | ':' | ';' | '-' | '–' | '—' => (Viseme::Rest, 120),
            '.' | '!' | '?' | '…' => (Viseme::Rest, 240),
            '\'' | '’' | '"' | '“' | '”' | '(' | ')' => continue,
            c if c.is_ascii_digit() => {
                return Err(AlignmentError(
                    "write Finnish numbers as spoken words or import reviewed mouth timings".into(),
                ))
            }
            c => {
                return Err(AlignmentError(format!(
                    "unsupported Finnish estimate character {c:?}; use reviewed timings"
                )))
            }
        };
        letters += usize::from(c.is_alphabetic());
        units.push((shape, ms));
    }
    if letters == 0 {
        return Err(AlignmentError("Finnish text contains no letters".into()));
    }
    let total: u64 = units.iter().map(|(_, ms)| ms).sum();
    let mut cues = Vec::with_capacity(units.len() + 2);
    push(&mut cues, 0, start, Viseme::Rest);
    let mut accumulated = 0u64;
    let mut at = start;
    for (shape, ms) in units {
        accumulated += ms;
        let until = start + ((end - start) as u128 * accumulated as u128 / total as u128) as u64;
        push(&mut cues, at, until, shape);
        at = until;
    }
    push(&mut cues, end, frames, Viseme::Rest);
    Ok(EstimatedVisemeTrack {
        source: source(),
        sample_rate: 24_000,
        frame_count: frames,
        cues,
        fallback_words: 0,
        time_scale: (end - start) as f64 / (total * 24) as f64,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn finnish_vowels_half_open_silence_and_identity_are_language_bound() {
        let t = estimate("Mää, yö!", 48000, 2400, 43200).unwrap();
        assert_eq!(t.viseme_at_sample(0), Viseme::Rest);
        assert_eq!(t.viseme_at_sample(2400), Viseme::Closed);
        assert_eq!(t.viseme_at_sample(43200), Viseme::Rest);
        assert!(t.cues.iter().any(|c| c.viseme == Viseme::U));
        assert!(t.cues.iter().any(|c| c.viseme == Viseme::O));
        for p in t.cues.windows(2) {
            assert_eq!(p[0].end_sample, p[1].start_sample);
        }
        assert_ne!(
            identity("mää", 48000, 2400, 43200),
            identity("maa", 48000, 2400, 43200)
        );
        assert!(matches!(t.source,AlignmentSource::Estimated{ref tool,..}if tool==TOOL));
    }
    #[test]
    fn invalid_input_never_uses_english() {
        for text in ["", "123", "Hei 😀", "日本語"] {
            assert!(estimate(text, 24000, 0, 24000).is_err());
        }
        assert!(estimate("Hei", 24000, 0, 24001).is_err());
        assert_eq!(silent_track(24000).unwrap().cues.len(), 1);
    }
}
