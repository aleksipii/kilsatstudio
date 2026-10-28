//! Finnish orthographic mouth estimate. No English lexicon or acoustic alignment.
use crate::{
    english::{EstimatedVisemeTrack, VisemeCue, MAX_TRACK_FRAMES},
    AlignmentError, AlignmentSource, Viseme,
};
use motion_audio::clock::DIALOGUE_SAMPLE_RATE as SAMPLE_RATE;
use sha2::{Digest, Sha256};
pub const TOOL: &str = "kilsat-finnish-orthographic-estimator";
/// Version 2: NFD composition, velar `n`, `š`/`ž`/`x` rules (output changed).
pub const VERSION: &str = "2";
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
        sample_rate: SAMPLE_RATE,
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
/// Lower-cases `text` and composes the decomposed (NFD) letters macOS text fields
/// and file names often produce, so `a` + U+0308 is the same letter as `ä`.
fn letters(text: &str) -> Result<Vec<char>, AlignmentError> {
    let mut out: Vec<char> = Vec::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        let composed = match (out.last().copied(), c) {
            (Some('a'), '\u{0308}') => 'ä',
            (Some('o'), '\u{0308}') => 'ö',
            (Some('u'), '\u{0308}') => 'ü',
            (Some('a'), '\u{030A}') => 'å',
            (Some('s'), '\u{030C}') => 'š',
            (Some('z'), '\u{030C}') => 'ž',
            (Some('e'), '\u{0301}') => 'é',
            (_, '\u{0300}'..='\u{036F}') => {
                return Err(AlignmentError(format!(
                    "unsupported combining mark U+{:04X} in Finnish text; use reviewed timings",
                    c as u32
                )))
            }
            _ => {
                out.push(c);
                continue;
            }
        };
        *out.last_mut().expect("matched a previous letter") = composed;
    }
    Ok(out)
}

/// Unscaled `(viseme, milliseconds)` units, adjacent equal shapes merged.
///
/// Finnish rules (docs/finnish-speech.md):
/// - `ä` → A, `ö` → O, `y` → U (front vowels share the nearest of the 13 shapes);
/// - a long vowel or geminate consonant is two letters and so lasts twice as long
///   as its short form (`tuli`/`tuuli`, `kato`/`katto`, `tapaa`/`tappaa`);
/// - `n` before `k` or `g` is the velar nasal /ŋ/ (`kenkä`, `kengät`) and, like
///   other velars, has no visible tongue tip: Rest;
/// - `š`, `ž` → Sh; `x` → /ks/; `w` → V; `z` → Sibilant; `c`, `q` → Rest.
pub(crate) fn units(text: &str) -> Result<Vec<(Viseme, u64)>, AlignmentError> {
    let letters = letters(text)?;
    let mut units: Vec<(Viseme, u64)> = Vec::with_capacity(letters.len());
    let mut add = |shape: Viseme, ms: u64| match units.last_mut() {
        Some(last) if last.0 == shape => last.1 += ms,
        _ => units.push((shape, ms)),
    };
    let mut spoken = 0usize;
    for (i, &c) in letters.iter().enumerate() {
        let next = letters.get(i + 1).copied();
        let (shape, ms) = match c {
            'a' | 'ä' | 'á' | 'à' => (Viseme::A, 100u64),
            'e' | 'é' | 'è' => (Viseme::E, 95),
            'i' | 'j' => (Viseme::I, 90),
            'o' | 'ö' | 'å' => (Viseme::O, 100),
            'u' | 'y' | 'ü' => (Viseme::U, 100),
            'p' | 'b' | 'm' => (Viseme::Closed, 65),
            'f' | 'v' | 'w' => (Viseme::TeethLip, 65),
            's' | 'z' => (Viseme::Sibilant, 70),
            'š' | 'ž' => (Viseme::Sh, 75),
            'n' if matches!(next, Some('k' | 'g')) => (Viseme::Rest, 60),
            't' | 'd' | 'n' | 'l' => (Viseme::TongueUp, 60),
            'r' => (Viseme::R, 65),
            'x' => {
                add(Viseme::Rest, 55);
                (Viseme::Sibilant, 70)
            }
            'k' | 'g' | 'h' | 'q' | 'c' => (Viseme::Rest, 55),
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
        spoken += usize::from(c.is_alphabetic());
        add(shape, ms);
    }
    if spoken == 0 {
        return Err(AlignmentError("Finnish text contains no letters".into()));
    }
    Ok(units)
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
    let units = units(text)?;
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
        sample_rate: SAMPLE_RATE,
        frame_count: frames,
        cues,
        fallback_words: 0,
        time_scale: (end - start) as f64 / (total * u64::from(SAMPLE_RATE / 1000)) as f64,
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
        assert_eq!(VERSION, "2");
        assert!(matches!(t.source,AlignmentSource::Estimated{ref tool,..}if tool==TOOL));
    }
    use Viseme::*;
    fn ms_of(text: &str, shape: Viseme) -> Vec<u64> {
        units(text)
            .unwrap()
            .into_iter()
            .filter(|(v, _)| *v == shape)
            .map(|(_, ms)| ms)
            .collect()
    }
    #[test]
    fn front_vowels_map_to_their_nearest_shape() {
        assert_eq!(units("ä").unwrap(), vec![(A, 100)]);
        assert_eq!(units("ö").unwrap(), vec![(O, 100)]);
        assert_eq!(units("y").unwrap(), vec![(U, 100)]);
        assert_eq!(units("Ä Ö Y").unwrap(), units("ä ö y").unwrap());
        // "yö": rounded front vowels, not spread A/E/I shapes.
        assert_eq!(units("yö").unwrap(), vec![(U, 100), (O, 100)]);
        assert_eq!(
            units("pöytä").unwrap()[..3],
            [(Closed, 65), (O, 100), (U, 100)]
        );
    }
    #[test]
    fn long_vowels_last_twice_as_long_as_short_ones() {
        assert_eq!(ms_of("tuli", U), vec![100]);
        assert_eq!(ms_of("tuuli", U), vec![200]);
        assert_eq!(ms_of("tapaa", A), vec![100, 200]);
        assert_eq!(ms_of("sää", A), vec![200]);
        assert_eq!(ms_of("työ", O), vec![100]);
        assert_eq!(ms_of("tyyny", U), vec![200, 100]);
        assert_eq!(ms_of("Pöö", O), vec![200]);
    }
    #[test]
    fn geminate_consonants_hold_their_closure_twice_as_long() {
        assert_eq!(ms_of("kato", TongueUp), vec![60]);
        assert_eq!(ms_of("katto", TongueUp), vec![120]);
        assert_eq!(ms_of("tapaa", Closed), vec![65]);
        assert_eq!(ms_of("tappaa", Closed), vec![130]);
        assert_eq!(ms_of("kissa", Sibilant), vec![140]);
        assert_eq!(ms_of("tuli", TongueUp), vec![60, 60]);
        assert_eq!(ms_of("tulli", TongueUp), vec![60, 120]);
        assert_eq!(ms_of("ruusu", R), vec![65]);
        assert_eq!(ms_of("varrella", R), vec![130]);
        // Geminate velar: no visible closure, but the held rest is still longer.
        assert_eq!(
            units("kukka").unwrap(),
            vec![(Rest, 55), (U, 100), (Rest, 110), (A, 100)]
        );
        // The estimate keeps the ratio after scaling to the measured span.
        let short = estimate("kato", 24_000, 0, 24_000).unwrap();
        let long = estimate("katto", 24_000, 0, 24_000).unwrap();
        let tongue = |t: &EstimatedVisemeTrack| {
            t.cues
                .iter()
                .filter(|c| c.viseme == TongueUp)
                .map(|c| c.end_sample - c.start_sample)
                .sum::<u64>()
        };
        assert!(tongue(&long) > tongue(&short) * 3 / 2);
    }
    #[test]
    fn n_before_k_or_g_is_the_velar_nasal() {
        assert_eq!(
            units("kenkä").unwrap(),
            vec![(Rest, 55), (E, 95), (Rest, 115), (A, 100)]
        );
        assert_eq!(
            units("kengät").unwrap(),
            vec![(Rest, 55), (E, 95), (Rest, 115), (A, 100), (TongueUp, 60)]
        );
        // Elsewhere n is alveolar.
        assert_eq!(ms_of("nenä", TongueUp), vec![60, 60]);
        assert_eq!(ms_of("Anna", TongueUp), vec![120]);
    }
    #[test]
    fn decomposed_letters_and_finnish_alphabet_extras() {
        assert_eq!(units("ma\u{0308}a\u{0308}").unwrap(), units("mää").unwrap());
        assert_eq!(units("yo\u{0308}").unwrap(), units("yö").unwrap());
        assert_eq!(units("A\u{030A}").unwrap(), units("å").unwrap());
        assert_eq!(units("s\u{030C}akki").unwrap(), units("šakki").unwrap());
        assert_eq!(units("šakki").unwrap()[0], (Sh, 75));
        assert_eq!(units("Žanna").unwrap()[0], (Sh, 75));
        assert_eq!(units("taksi").unwrap(), units("taxi").unwrap());
        assert_eq!(units("wc").unwrap(), vec![(TeethLip, 65), (Rest, 55)]);
        assert!(units("n\u{0303}").is_err()); // ñ via combining tilde: unsupported.
        assert!(units("\u{0308}").is_err()); // bare combining mark.
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
