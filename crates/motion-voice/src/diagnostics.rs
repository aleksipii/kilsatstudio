//! Non-fatal, typed preparation diagnostics. Headless: no UI types.
use crate::timeline::TextEstimateInfo;

/// Audio length over estimated model length outside this range is reported as a poor
/// fit (lead-in silence, very slow or fast reading). A heuristic, not a quality measure.
pub const POOR_FIT_SCALE: std::ops::RangeInclusive<f64> = 0.4..=2.5;

#[derive(Clone, Debug, PartialEq)]
pub enum PreparationDiagnostic {
    /// Words not in the lexicon were pronounced by the rule-based fallback; add them
    /// under `pronunciations:` for a better text estimate.
    LowConfidencePronunciation {
        dialogue_id: u64,
        actor: String,
        words: Vec<String>,
    },
    /// The text estimate does not fit the audio length well.
    PoorTextEstimateFit {
        dialogue_id: u64,
        actor: String,
        time_scale: f64,
    },
    /// No text estimate could be made (e.g. no speakable words); the mouth stays at rest.
    TextEstimateUnavailable {
        dialogue_id: u64,
        actor: String,
        reason: String,
    },
}

impl PreparationDiagnostic {
    pub fn dialogue_id(&self) -> u64 {
        match self {
            Self::LowConfidencePronunciation { dialogue_id, .. }
            | Self::PoorTextEstimateFit { dialogue_id, .. }
            | Self::TextEstimateUnavailable { dialogue_id, .. } => *dialogue_id,
        }
    }
    pub fn message(&self) -> String {
        match self {
            Self::LowConfidencePronunciation {
                dialogue_id,
                actor,
                words,
            } => format!(
                "Dialogue {dialogue_id} ({actor}): low-confidence pronunciation guessed for {}; add `pronunciations:` overrides. Mouth shapes are text-estimated, not aligned to audio.",
                words.join(", ")
            ),
            Self::PoorTextEstimateFit {
                dialogue_id,
                actor,
                time_scale,
            } => format!(
                "Dialogue {dialogue_id} ({actor}): text estimate was stretched {time_scale:.2}x to fit the audio; timing is likely poor (silence or unusual pace)."
            ),
            Self::TextEstimateUnavailable {
                dialogue_id,
                actor,
                reason,
            } => format!("Dialogue {dialogue_id} ({actor}): no text-estimated mouth track ({reason}); mouth stays at rest."),
        }
    }
}

/// Diagnostics for one derived estimate.
pub fn from_estimate(
    dialogue_id: u64,
    actor: &str,
    info: &TextEstimateInfo,
) -> Vec<PreparationDiagnostic> {
    let mut out = Vec::new();
    if !info.low_confidence_words.is_empty() {
        out.push(PreparationDiagnostic::LowConfidencePronunciation {
            dialogue_id,
            actor: actor.into(),
            words: info.low_confidence_words.clone(),
        });
    }
    if !POOR_FIT_SCALE.contains(&info.time_scale) {
        out.push(PreparationDiagnostic::PoorTextEstimateFit {
            dialogue_id,
            actor: actor.into(),
            time_scale: info.time_scale,
        });
    }
    out
}
