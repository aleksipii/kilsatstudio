use motion_lipsync::{audio_timeline::AudioTimeline, AlignmentSource, Viseme};
use std::path::PathBuf;

fn example(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/audio-timeline-v1")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

#[test]
fn valid_examples_load_and_evaluate() {
    let fi = AudioTimeline::from_json(&example("fi-manual.audio-timeline.json")).unwrap();
    let clip = &fi.clips[0];
    assert_eq!(clip.mouth.as_ref().unwrap().source, AlignmentSource::Manual);
    // Scene 12000 + local 1900 = "y"; local 7600 = "ø"; gap at local 6200..6600 rests.
    assert_eq!(fi.viseme_at("AINO", 13_900), Viseme::U);
    assert_eq!(fi.viseme_at("AINO", 19_600), Viseme::O);
    assert_eq!(fi.viseme_at("AINO", 18_300), Viseme::Rest);
    // Every manual phoneme agrees with the viseme stored for it.
    let mouth = clip.mouth.as_ref().unwrap();
    for (p, v) in mouth.phonemes.iter().zip(&mouth.visemes) {
        assert_eq!(
            motion_lipsync::phoneme_to_viseme(&p.phoneme),
            Some(v.viseme)
        );
        assert_eq!(
            (p.start_sample, p.end_sample),
            (v.start_sample, v.end_sample)
        );
    }
    let en = AudioTimeline::from_json(&example("en-estimated.audio-timeline.json")).unwrap();
    // Start 2400, trim 2400: scene s plays local s, so "u" starts at scene 3600.
    assert_eq!(en.viseme_at("EMMA", 3_599), Viseme::Rest);
    assert_eq!(en.viseme_at("EMMA", 3_600), Viseme::U);
    assert_eq!(en.clips[0].source_sample_at(2_399), None);
    assert_eq!(en.clips[0].source_sample_at(19_199), Some(19_199));
    // Serialization is stable: re-encoding and decoding gives the same value.
    for t in [fi, en] {
        assert_eq!(
            AudioTimeline::from_json(t.to_json_pretty().unwrap().as_bytes()).unwrap(),
            t
        );
    }
}

#[test]
fn invalid_examples_fail_with_specific_errors() {
    for (name, needle) in [
        ("invalid-overlapping-actor.audio-timeline.json", "overlap"),
        (
            "invalid-trim-exceeds-source.audio-timeline.json",
            "trim and duration exceed",
        ),
        (
            "invalid-future-version.audio-timeline.json",
            "format_version 2",
        ),
        (
            "invalid-unknown-phoneme.audio-timeline.json",
            "unsupported IPA phoneme",
        ),
    ] {
        let error = AudioTimeline::from_json(&example(name)).unwrap_err();
        assert!(error.contains(needle), "{name}: {error}");
    }
}
