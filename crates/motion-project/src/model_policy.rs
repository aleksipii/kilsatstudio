//! Project/job model policy (ADR-A01-03, ADR-A01-09). One check point decides whether
//! a model-backed step may start. Storing the choice in `ProjectDocument` and the UI
//! check points belong to A09; this module defines the type and its rules only.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelPolicy {
    /// Current behaviour: the user may run local models (Kokoro, Piper, recognizers)
    /// while preparing audio. Animation still evaluates rules only.
    #[default]
    LocalPreprocessing,
    /// Strict model-free path: imported WAV, hand-authored or rule-estimated timing,
    /// rule-based evaluation and rendering. No model runs or downloads, no fallback.
    Strict,
}

/// Steps that run or install a model. Rule-based estimates are not listed: they
/// are allowed on both paths and labelled `Estimated`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelBackedStep {
    SpeechSynthesis,
    RecognizerAlignment,
    CameraFaceTracking,
    ModelDownload,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
#[error("{step:?} is blocked by the strict model-free policy; {hint}")]
pub struct PolicyViolation {
    pub step: ModelBackedStep,
    pub hint: &'static str,
}

impl ModelPolicy {
    /// The only check point. Call before building a provider, starting a recognizer
    /// or camera process, or installing assets.
    pub fn permit(self, step: ModelBackedStep) -> Result<(), PolicyViolation> {
        match self {
            Self::LocalPreprocessing => Ok(()),
            Self::Strict => Err(PolicyViolation {
                step,
                hint: match step {
                    ModelBackedStep::SpeechSynthesis => "import a 24 kHz mono WAV for each line",
                    ModelBackedStep::RecognizerAlignment => {
                        "author mouth timing by hand or use the rule-based estimate"
                    }
                    ModelBackedStep::CameraFaceTracking => "animate the head by hand",
                    ModelBackedStep::ModelDownload => "no model assets are installed on this path",
                },
            }),
        }
    }

    pub fn is_strict(self) -> bool {
        self == Self::Strict
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const STEPS: [ModelBackedStep; 4] = [
        ModelBackedStep::SpeechSynthesis,
        ModelBackedStep::RecognizerAlignment,
        ModelBackedStep::CameraFaceTracking,
        ModelBackedStep::ModelDownload,
    ];

    #[test]
    fn strict_blocks_every_model_step_and_default_allows_them() {
        for step in STEPS {
            assert_eq!(ModelPolicy::default().permit(step), Ok(()));
            let error = ModelPolicy::Strict.permit(step).unwrap_err();
            assert_eq!(error.step, step);
            assert!(error.to_string().contains("strict model-free policy"));
        }
    }

    #[test]
    fn serialized_names_are_stable() {
        assert_eq!(
            serde_json::to_string(&ModelPolicy::Strict).unwrap(),
            "\"strict\""
        );
        assert_eq!(
            serde_json::from_str::<ModelPolicy>("\"local_preprocessing\"").unwrap(),
            ModelPolicy::LocalPreprocessing
        );
        assert!(serde_json::from_str::<ModelPolicy>("\"cloud\"").is_err());
    }
}
