//! Bounded, deterministic automatic direction for editable two-character scenes.
//! Estimated timing is a draft, never measured speech or a quality certificate.
mod acting;
mod conversational;
mod director;
mod orchestrator;
mod parser;
mod timing;

pub use conversational::{direct_conversational, direct_with_audio};
pub use orchestrator::direct;
pub use parser::parse_dialogue;
pub use timing::time_lines;

pub const SAMPLE_RATE: u64 = 24_000;
/// Whole-screenplay input/output budget, also shared by the desktop editor.
pub const MAX_SOURCE_BYTES: usize = 256 * 1024;
pub const MAX_LINES: usize = 16;
pub const MAX_TEXT_BYTES: usize = 16_000;
pub const MAX_DURATION_SAMPLES: u64 = 60 * SAMPLE_RATE;
pub const LEAD_SAMPLES: u64 = SAMPLE_RATE;
pub const GAP_SAMPLES: u64 = SAMPLE_RATE / 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentLine {
    pub actor: String,
    pub text: String,
    pub emotion: Option<String>,
    pub source_line: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingSource {
    Estimated,
    SuppliedSamples,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedLine {
    pub line: IntentLine,
    pub start_sample: u64,
    pub duration_samples: u64,
    pub timing_source: TimingSource,
}

#[derive(Debug, Clone)]
pub struct DirectionResult {
    pub source: String,
    pub first_scene: usize,
    pub shot_count: usize,
    pub duration_samples: u64,
    pub timing_source: TimingSource,
    pub diagnostics: Vec<String>,
}
