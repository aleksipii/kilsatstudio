//! Content identity of a text-estimated viseme track, for cache invalidation.
use super::{Pronunciations, DIALOGUE_SAMPLE_RATE, TOOL_NAME, TOOL_VERSION};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

/// SHA-256 of the embedded CMUdict data plus the estimator tool name and version.
/// Changes whenever the lexicon data or the estimator model changes, so any identity
/// that includes it invalidates stale estimates.
pub fn lexicon_version() -> &'static str {
    static CELL: OnceLock<String> = OnceLock::new();
    CELL.get_or_init(|| {
        let mut h = Sha256::new();
        h.update(super::lexicon::Lexicon::data_bytes());
        h.update(TOOL_NAME.as_bytes());
        h.update(b"\0");
        h.update(TOOL_VERSION.as_bytes());
        format!("{TOOL_NAME}-{TOOL_VERSION}-{:x}", h.finalize())
    })
}

/// Identity of `estimate_viseme_track(text, overrides, Some(audio_frames))`: covers the
/// text, every pronunciation override, the lexicon/estimator version, sample rate and
/// audio length. Equal identities imply equal tracks; any change invalidates.
pub fn estimate_identity(text: &str, overrides: &Pronunciations, audio_frames: u64) -> String {
    identity(text, overrides, audio_frames, None)
}

/// Identity of `estimate_viseme_track_in_span`. Equals [`estimate_identity`] when the
/// span covers the whole clip (the tracks are then identical); otherwise it also
/// covers the span, so a re-synthesized clip with different silence invalidates.
pub fn estimate_identity_in_span(
    text: &str,
    overrides: &Pronunciations,
    audio_frames: u64,
    speech_start: u64,
    speech_end: u64,
) -> String {
    let full = speech_start == 0 && speech_end == audio_frames;
    identity(
        text,
        overrides,
        audio_frames,
        (!full).then_some((speech_start, speech_end)),
    )
}

fn identity(
    text: &str,
    overrides: &Pronunciations,
    audio_frames: u64,
    span: Option<(u64, u64)>,
) -> String {
    let mut h = Sha256::new();
    let mut field = |tag: &[u8], bytes: &[u8]| {
        h.update(tag);
        h.update((bytes.len() as u64).to_le_bytes());
        h.update(bytes);
    };
    field(b"lexicon", lexicon_version().as_bytes());
    field(b"rate", &DIALOGUE_SAMPLE_RATE.to_le_bytes());
    field(b"frames", &audio_frames.to_le_bytes());
    field(b"text", text.as_bytes());
    for (word, phones) in overrides.iter() {
        field(b"word", word.as_bytes());
        let joined: Vec<&str> = phones.iter().map(|p| p.symbol()).collect();
        field(b"phones", joined.join(" ").as_bytes());
    }
    if let Some((start, end)) = span {
        field(b"speech_start", &start.to_le_bytes());
        field(b"speech_end", &end.to_le_bytes());
    }
    format!("{:x}", h.finalize())
}
