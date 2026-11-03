//! Finnish character voices derived from the one Finnish Piper speaker.
//!
//! The public Piper voice set has a single Finnish speaker (`fi_FI-harri`).
//! Each character voice here is that speaker synthesized at a different
//! length and resampled back to the requested duration: pitch and formants
//! move together by `pitch` (about ±3–5 semitones), so the voice sounds
//! deeper and larger or higher and smaller while speaking at the same rate.
//! They are variants of one recorded voice, not separate speakers.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FinnishVoice {
    pub preset: &'static str,
    /// Piper preset of the speaker model it is derived from.
    pub base: &'static str,
    /// Finnish label for editors and guides.
    pub label: &'static str,
    /// Pitch and formant factor in thousandths (1000 = unchanged).
    pub pitch_permille: u32,
}

/// Every Finnish preset, the unchanged speaker first.
pub const FINNISH_VOICES: [FinnishVoice; 5] = [
    FinnishVoice {
        preset: "piper_harri",
        base: "piper_harri",
        label: "Harri – perusääni",
        pitch_permille: 1000,
    },
    FinnishVoice {
        preset: "piper_harri_jykea",
        base: "piper_harri",
        label: "Jykeä – hyvin matala (−5 sävelaskelta)",
        pitch_permille: 749,
    },
    FinnishVoice {
        preset: "piper_harri_matala",
        base: "piper_harri",
        label: "Matala (−3 sävelaskelta)",
        pitch_permille: 841,
    },
    FinnishVoice {
        preset: "piper_harri_korkea",
        base: "piper_harri",
        label: "Korkea (+3 sävelaskelta)",
        pitch_permille: 1189,
    },
    FinnishVoice {
        preset: "piper_harri_kirkas",
        base: "piper_harri",
        label: "Kirkas – hyvin korkea (+5 sävelaskelta)",
        pitch_permille: 1335,
    },
];

/// The Finnish voice of `preset`, if it is one.
pub fn finnish_voice(preset: &str) -> Option<&'static FinnishVoice> {
    FINNISH_VOICES.iter().find(|v| v.preset == preset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_unique_valid_and_semitone_spaced() {
        let mut seen = std::collections::BTreeSet::new();
        for voice in FINNISH_VOICES {
            assert!(seen.insert(voice.preset));
            assert!(voice.preset.starts_with("piper_"));
            assert!(voice
                .preset
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_'));
            assert!(finnish_voice(voice.base).is_some_and(|b| b.pitch_permille == 1000));
            // Each factor is 2^(n/12) for a whole number of semitones.
            let semitones = 12.0 * (f64::from(voice.pitch_permille) / 1000.0).log2();
            assert!(
                (semitones - semitones.round()).abs() < 0.01,
                "{}",
                voice.preset
            );
        }
        assert!(finnish_voice("piper_unknown").is_none());
    }
}
