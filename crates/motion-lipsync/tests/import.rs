use motion_lipsync::{
    arpabet_to_ipa, import_alignment_json, AlignmentAudio, AlignmentSource, Viseme,
};

const FIXTURE: &str = include_str!("fixtures/external.json");
fn audio() -> AlignmentAudio {
    AlignmentAudio {
        asset_id: "sha256:example-audio".into(),
        sample_rate: 24000,
        frames: 24000,
    }
}
#[test]
fn imported_times_and_provenance_are_preserved() {
    let alignment = import_alignment_json(FIXTURE, &audio()).unwrap();
    assert!(
        matches!(alignment.source, AlignmentSource::Forced { ref tool, ref version } if tool == "fixture-aligner" && version == "test-only")
    );
    assert_eq!(alignment.duration_seconds, 1.0);
    assert_eq!(alignment.phonemes[1].phoneme, "ɑ");
    assert_eq!(alignment.phonemes[1].start_seconds, 0.3);
    assert_eq!(alignment.viseme_at(0.3).unwrap(), Viseme::A);
    assert_eq!(alignment.viseme_at(0.8).unwrap(), Viseme::Rest);
}
#[test]
fn wrong_audio_and_invalid_audio_are_rejected() {
    let mut expected = audio();
    expected.asset_id = "other-take".into();
    assert!(import_alignment_json(FIXTURE, &expected)
        .unwrap_err()
        .0
        .contains("mismatch"));
    let mut expected = audio();
    expected.frames -= 1;
    assert!(import_alignment_json(FIXTURE, &expected).is_err());
    let mut expected = audio();
    expected.sample_rate = 0;
    assert!(import_alignment_json(FIXTURE, &expected).is_err());
}
#[test]
fn malformed_unknown_or_out_of_range_imports_fail() {
    for invalid in [
        FIXTURE.replace("\"schema_version\": 1", "\"schema_version\": 2"),
        FIXTURE.replace("\"alphabet\": \"arpabet\"", "\"alphabet\": \"words\""),
        FIXTURE.replace("\"AA1\"", "\"hello\""),
        FIXTURE.replace("\"tool_version\": \"test-only\"", "\"tool_version\": \"\""),
        FIXTURE.replace("\"end_seconds\": 1.0", "\"end_seconds\": 1.1"),
        FIXTURE.replace("\"start_seconds\": 0.3", "\"start_seconds\": 0.2"),
        FIXTURE.replace(
            "\"schema_version\": 1",
            "\"unexpected\": true, \"schema_version\": 1",
        ),
    ] {
        assert!(
            import_alignment_json(&invalid, &audio()).is_err(),
            "{invalid}"
        );
    }
}
#[test]
fn empty_phone_tier_is_actionable() {
    let mut document: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
    document["phonemes"] = serde_json::json!([]);
    assert!(import_alignment_json(&document.to_string(), &audio())
        .unwrap_err()
        .0
        .contains("phone tier"));
}
#[test]
fn arpabet_stress_and_inventory_are_explicit() {
    assert_eq!(arpabet_to_ipa("AH0"), Some("ə"));
    assert_eq!(arpabet_to_ipa("ER1"), Some("ɝ"));
    assert_eq!(arpabet_to_ipa("CH"), Some("tʃ"));
    for unknown in ["B1", "AA3", "AX", "spn", "aa", "word"] {
        assert_eq!(arpabet_to_ipa(unknown), None);
    }
    for phone in [
        "AA", "AE", "AH", "AO", "AW", "AY", "EH", "ER", "EY", "IH", "IY", "OW", "OY", "UH", "UW",
        "B", "CH", "D", "DH", "F", "G", "HH", "JH", "K", "L", "M", "N", "NG", "P", "R", "S", "SH",
        "T", "TH", "V", "W", "Y", "Z", "ZH", "sil", "sp",
    ] {
        assert!(motion_lipsync::phoneme_to_viseme(arpabet_to_ipa(phone).unwrap()).is_some());
    }
}
