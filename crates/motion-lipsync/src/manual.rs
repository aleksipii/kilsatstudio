//! Explicit human edits to mouth shapes. Representative labels encode visemes;
//! they are not a phonetic transcription or a forced-alignment result.
use crate::{
    cache::{BoundAlignment, CacheKey},
    rhubarb::{EstimatedVisemeAlignment, MouthCue},
    Alignment, AlignmentError, AlignmentSource, TimedPhoneme,
};
use motion_audio::AudioBuffer;
use sha2::{Digest, Sha256};

/// The desktop editor has a stricter budget than imported recognizer output.
pub const MAX_EDIT_CUES: usize = 1024;

pub fn validate_cues(cues: &[MouthCue], frames: u64) -> Result<(), AlignmentError> {
    if frames == 0 || cues.len() > MAX_EDIT_CUES {
        return Err(AlignmentError("manual timing exceeds its cue/audio budget".into()));
    }
    let mut previous = 0;
    for cue in cues {
        if cue.start_sample < previous
            || cue.end_sample <= cue.start_sample
            || cue.end_sample > frames
            || crate::rhubarb::shape_viseme(cue.shape).is_none()
        {
            return Err(AlignmentError(
                "manual cues require ordered non-overlapping sample intervals within audio and shapes A–H/X".into(),
            ));
        }
        previous = cue.end_sample;
    }
    Ok(())
}

/// Binds the explicit edit to exact current PCM/text and original provenance.
/// Run on a worker: validating/hashing the prepared PCM can be expensive.
pub fn bind_edits(
    original: &EstimatedVisemeAlignment,
    cues: &[MouthCue],
    audio: &AudioBuffer,
    transcript: &str,
) -> Result<BoundAlignment, AlignmentError> {
    original.validate_for(audio, transcript)?;
    validate_cues(cues, original.frame_count)?;
    let encoded = serde_json::to_vec(cues).map_err(|e| AlignmentError(e.to_string()))?;
    let edit_identity = format!("{:x}", Sha256::digest(&encoded));
    // Include the complete original tool/PCM/transcript/cue identity, not a path.
    let origin = serde_json::to_vec(original).map_err(|e| AlignmentError(e.to_string()))?;
    let configuration = format!(
        "manual-mouth-shapes-v1; original_rhubarb_sha256={:x}; tool={}; recognizer={:?}",
        Sha256::digest(origin),
        original.tool_version,
        original.recognizer
    );
    let key = CacheKey::new(
        audio,
        transcript,
        "kilsat-manual-mouth-editor",
        "1",
        &configuration,
        &edit_identity,
    )?;
    let rate = f64::from(audio.sample_rate);
    let phonemes = cues
        .iter()
        .map(|cue| TimedPhoneme {
            // Existing deterministic evaluator uses the phoneme-to-viseme map.
            // These are representative viseme carriers, explicitly Manual.
            phoneme: match cue.shape {
                'A' => "m",
                'B' => "i",
                'C' => "e",
                'D' => "a",
                'E' => "o",
                'F' => "u",
                'G' => "f",
                'H' => "l",
                _ => "sil",
            }
            .into(),
            start_seconds: cue.start_sample as f64 / rate,
            end_seconds: cue.end_sample as f64 / rate,
        })
        .collect();
    BoundAlignment::new(
        key,
        Some(Alignment {
            source: AlignmentSource::Manual,
            duration_seconds: audio.frames() as f64 / rate,
            phonemes,
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{cache::AlignmentInput, rhubarb::Recognizer, Viseme};

    fn fixture() -> (AudioBuffer, EstimatedVisemeAlignment) {
        let audio = AudioBuffer {
            sample_rate: 24000,
            channels: 1,
            samples: vec![0.0; 240],
        };
        let original = EstimatedVisemeAlignment {
            schema_version: 1,
            source: "estimated_rhubarb".into(),
            tool_version: "fixture-1".into(),
            executable_sha256: "a".repeat(64),
            pcm_sha256: crate::rhubarb::pcm_identity(&audio).unwrap(),
            transcript_sha256: format!("{:x}", Sha256::digest(b"hello")),
            recognizer: Recognizer::Phonetic,
            sample_rate: 24000,
            frame_count: 240,
            cues: vec![],
        };
        (audio, original)
    }

    #[test]
    fn edits_preserve_manual_provenance_exact_boundaries_and_pcm_binding() {
        let (mut audio, original) = fixture();
        let cues = vec![MouthCue { start_sample: 10, end_sample: 20, shape: 'D' }];
        let bound = bind_edits(&original, &cues, &audio, "hello").unwrap();
        let alignment = bound.alignment.as_ref().unwrap();
        assert!(matches!(alignment.source, AlignmentSource::Manual));
        assert_eq!(alignment.viseme_at(9.0 / 24000.0).unwrap(), Viseme::Rest);
        assert_eq!(alignment.viseme_at(10.0 / 24000.0).unwrap(), Viseme::A);
        assert_eq!(alignment.viseme_at(20.0 / 24000.0).unwrap(), Viseme::Rest);
        let text = serde_json::to_string(&AlignmentInput::Bound(bound)).unwrap();
        let input: AlignmentInput = serde_json::from_str(&text).unwrap();
        let root = std::env::temp_dir().join(format!("kilsat-manual-test-{}", std::process::id()));
        let cache = crate::cache::AlignmentCache::new(root.clone());
        assert!(input.resolve(&audio, "hello", &cache).is_ok());
        assert!(input.resolve(&audio, "changed", &cache).is_err());
        audio.samples[0] = 0.5;
        assert!(input.resolve(&audio, "hello", &cache).is_err());
        assert!(bind_edits(&original, &cues, &audio, "hello").is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn every_shape_keeps_its_evaluator_meaning_and_each_edit_has_a_distinct_key() {
        let (audio, original) = fixture();
        let mut cues: Vec<_> = ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'X']
            .into_iter()
            .enumerate()
            .map(|(index, shape)| MouthCue {
                start_sample: index as u64 * 4,
                end_sample: index as u64 * 4 + 2,
                shape,
            })
            .collect();
        let first = bind_edits(&original, &cues, &audio, "hello").unwrap();
        for cue in &cues {
            assert_eq!(
                first.alignment.as_ref().unwrap().viseme_at(cue.start_sample as f64 / 24000.0).unwrap(),
                crate::rhubarb::shape_viseme(cue.shape).unwrap()
            );
        }
        cues[0].end_sample += 1;
        let second = bind_edits(&original, &cues, &audio, "hello").unwrap();
        assert_ne!(first.key, second.key);
        let mut changed_origin = original;
        changed_origin.tool_version = "fixture-2".into();
        let third = bind_edits(&changed_origin, &cues, &audio, "hello").unwrap();
        assert_ne!(second.key, third.key);
    }
    #[test]
    fn malformed_and_oversized_edits_fail_without_reordering() {
        for cue in [
            MouthCue { start_sample: 20, end_sample: 20, shape: 'A' },
            MouthCue { start_sample: 20, end_sample: 241, shape: 'A' },
            MouthCue { start_sample: 0, end_sample: 1, shape: '?' },
        ] {
            assert!(validate_cues(&[cue], 240).is_err());
        }
        let cue = MouthCue { start_sample: 0, end_sample: 1, shape: 'X' };
        assert!(validate_cues(&[cue.clone(), cue.clone()], 240).is_err());
        assert!(validate_cues(&vec![cue; MAX_EDIT_CUES + 1], 240).is_err());
        assert!(validate_cues(&[], 240).is_ok()); // Explicit all-rest edit.
    }
}
