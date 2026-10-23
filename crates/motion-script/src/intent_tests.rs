use super::*;
use serde_json::Value;

const FI: &str = include_str!("../../../examples/automation/pilot-fi.txt");
const EN: &str = include_str!("../../../examples/automation/pilot-en.txt");
const FI_INTENT: &str = include_str!("../../../examples/automation/pilot-intent.json");
const EN_INTENT: &str = include_str!("../../../examples/automation/pilot-en-intent.json");

/// Fixture fields owned by later stages (direction / camera), not by intake.
const DIRECTION_OWNED: [&str; 2] = ["shot_proposal", "app_insert"];
/// Intake fields that the hand-made fixtures predate.
const INTAKE_ONLY: [&str; 1] = ["unresolved"];

fn opts(language: IntentLanguage) -> IntakeOptions {
    IntakeOptions {
        language,
        seed: 31,
        scene_id: "varikko".into(),
    }
}

fn fi(src: &str) -> Result<Intake, IntakeFailure> {
    script_intent_from_text(src.as_bytes(), &opts(IntentLanguage::FiFi))
}

fn en(src: &str) -> Result<Intake, IntakeFailure> {
    script_intent_from_text(src.as_bytes(), &opts(IntentLanguage::EnGb))
}

fn kinds(r: Result<Intake, IntakeFailure>) -> Vec<IntakeErrorKind> {
    r.expect_err("expected failure")
        .errors
        .iter()
        .map(|e| e.kind)
        .collect()
}

const EN_HEAD: &str =
    "T\nDuration: 30 seconds. Location: room.\nCharacters: Aino, worker; Rane, boss.\n";
const FI_HEAD: &str =
    "T\nKesto: 30 sekuntia. Paikka: huone.\nHahmot: Aino, työntekijä; Rane, esihenkilö.\n";

fn assert_matches_fixture(src: &str, fixture: &str, language: IntentLanguage) {
    let intake = script_intent_from_text(src.as_bytes(), &opts(language)).unwrap();
    let mut produced: Value = serde_json::from_str(&intake.intent.to_json_pretty()).unwrap();
    let mut expected: Value = serde_json::from_str(fixture).unwrap();
    for k in DIRECTION_OWNED {
        assert!(expected.as_object_mut().unwrap().remove(k).is_some(), "{k}");
    }
    for k in INTAKE_ONLY {
        produced.as_object_mut().unwrap().remove(k).unwrap();
    }
    assert_eq!(produced, expected);
    intake.intent.verify_against(src.as_bytes()).unwrap();
}

#[test]
fn finnish_pilot_matches_fixture_field_for_field() {
    assert_matches_fixture(FI, FI_INTENT, IntentLanguage::FiFi);
}

#[test]
fn english_pilot_matches_fixture_field_for_field() {
    assert_matches_fixture(EN, EN_INTENT, IntentLanguage::EnGb);
}

#[test]
fn pilot_action_lines_are_unresolved_at_their_source_spans() {
    let intake = fi(FI).unwrap();
    let u = &intake.intent.unresolved;
    assert_eq!(u.len(), 3);
    assert!(u.iter().all(|x| x.code == UnresolvedCode::UnsupportedAction
        && x.status == ProvenanceKind::NeedsReview
        && x.rule_id == "ING-004"));
    assert_eq!(
        u[0].source_text,
        "Aino pukee takkia. Rane työntää avaimet pöydän yli."
    );
    // The app-screen line is preserved verbatim, not rendered or interpreted.
    assert!(u[1]
        .source_text
        .starts_with("Aino näyttää Kilsat – Ajopäiväkirja"));
    assert_eq!(
        (u[1].start_s.unwrap().millis(), u[1].end_s.unwrap().millis()),
        (24_000, 29_000)
    );
    for x in u {
        assert_eq!(x.source_span.slice(FI), Some(x.source_text.as_str()));
    }
    let h = &intake.header;
    assert_eq!(
        h.title,
        "OMALLA AJALLA — uusi suunnittelupilotti: Ihan pieni keikka"
    );
    assert_eq!(h.location, "varikon kahvihuone");
    assert_eq!(h.characters[0].role.as_deref(), Some("huoltotyöntekijä"));
    assert_eq!(h.characters[1].role.as_deref(), Some("esihenkilö"));
    assert_eq!(h.location_span.slice(FI), Some("varikon kahvihuone"));
}

#[test]
fn finnish_byte_ranges_are_utf8_byte_offsets() {
    let intake = fi(FI).unwrap();
    let b = &intake.intent.beats[2];
    assert_eq!(b.text.as_deref(), Some("Älä laske jokaista kilometriä. Täällä tarvitaan joustavaa asennetta ja yhteistä vastuuta."));
    // 89 characters but 96 bytes because of ä/Ä.
    assert_eq!(b.source_text.chars().count(), 89);
    assert_eq!(b.source_span.end_byte - b.source_span.start_byte, 96);
    assert_eq!(
        &FI.as_bytes()[b.source_span.start_byte..b.source_span.end_byte],
        b.source_text.as_bytes()
    );
}

#[test]
fn finnish_speaker_ids_and_multibyte_names() {
    let src = "T\nKesto: 30 sekuntia. Paikka: sauna.\nHahmot: Äijä, isä; Öljy-Åsa.\n[0–30] ÄIJÄ: Löyly on hyvä.\n";
    let intake = fi(src).unwrap();
    assert_eq!(intake.intent.characters[0].id, "ÄIJÄ");
    assert_eq!(intake.intent.characters[1].id, "ÖLJY-ÅSA");
    assert_eq!(intake.header.characters[1].role, None);
    let b = &intake.intent.beats[0];
    assert_eq!(b.source_span.slice(src), Some("Löyly on hyvä."));
}

#[test]
fn output_is_deterministic_and_seed_does_not_change_meaning() {
    let a = fi(FI).unwrap().intent.to_json_pretty();
    let b = fi(FI).unwrap().intent.to_json_pretty();
    assert_eq!(a, b);
    let mut o = opts(IntentLanguage::FiFi);
    o.seed = 7;
    let c = script_intent_from_text(FI.as_bytes(), &o).unwrap().intent;
    let d = fi(FI).unwrap().intent;
    assert_eq!(c.beats, d.beats);
    assert_eq!(c.unresolved, d.unresolved);
    assert_eq!(c.seed, 7);
}

#[test]
fn changed_source_rejects_old_spans() {
    let intent = fi(FI).unwrap().intent;
    let edited = FI.replace("Tämä on ihan pieni", "Tämä on ihan iso");
    let e = intent.verify_against(edited.as_bytes()).unwrap_err();
    assert_eq!(e.kind, IntakeErrorKind::StaleSource);
    let mut forged = intent.clone();
    forged.source_sha256 = sha256_hex(edited.as_bytes());
    assert_eq!(
        forged.verify_against(edited.as_bytes()).unwrap_err().kind,
        IntakeErrorKind::StaleSource
    );
}

#[test]
fn crlf_and_blank_lines_are_tolerated_without_shifting_spans() {
    let src = format!("{EN_HEAD}\n[0–30] AINO: Hello there.\n").replace('\n', "\r\n");
    let intake = en(&src).unwrap();
    assert_eq!(
        intake.intent.beats[0].source_span.slice(&src),
        Some("Hello there.")
    );
}

#[test]
fn fractional_times_serialize_as_decimal_seconds() {
    let src = format!("{EN_HEAD}[0–2.5] AINO: Hi.\n[2.5–30] RANE: Hello.\n");
    let json = en(&src).unwrap().intent.to_json_pretty();
    assert!(json.contains("\"end_s\": 2.5"), "{json}");
    assert!(
        json.contains("\"end_s\": 30\n") || json.contains("\"end_s\": 30,"),
        "{json}"
    );
}

#[test]
fn bounded_action_vocabulary_is_derived_with_provenance() {
    let src = format!("{FI_HEAD}[0–5] Aino katsoo Ranea.\n[5–10] Rane näyttää puhelimen.\n[10–12] Tauko.\n[12–30] AINO: Selvä.\n");
    let beats = fi(&src).unwrap().intent.beats;
    assert_eq!(beats[0].action, Some(IntentAction::LookAt));
    assert_eq!(beats[0].actor.as_deref(), Some("AINO"));
    assert_eq!(beats[0].target.as_deref(), Some("RANE"));
    assert_eq!(beats[0].provenance.kind, ProvenanceKind::Derived);
    assert_eq!(beats[1].action, Some(IntentAction::PresentProp));
    assert_eq!(beats[2].kind, BeatKind::Hold);
    assert_eq!(beats[2].actor, None);
    assert_eq!(beats[3].id, "b04");
    let src = format!("{EN_HEAD}[0–5] Rane looks at Aino.\n[5–30] Aino shows her phone.\n");
    let beats = en(&src).unwrap().intent.beats;
    assert_eq!(beats[0].target.as_deref(), Some("AINO"));
    assert_eq!(beats[1].target.as_deref(), Some("phone"));
}

#[test]
fn looking_at_an_unknown_target_or_compound_action_needs_review() {
    let src = format!("{EN_HEAD}[0–5] Rane looks at the door.\n[5–10] Aino looks at Rane. Rane sighs.\n[10–30] AINO: Ok.\n");
    let intent = en(&src).unwrap().intent;
    assert_eq!(intent.beats.len(), 1);
    assert_eq!(intent.unresolved.len(), 2);
}

#[test]
fn underfilled_timeline_is_flagged_at_the_duration() {
    let src = format!("{EN_HEAD}[0–20] AINO: Short.\n");
    let intent = en(&src).unwrap().intent;
    let u = intent.unresolved.last().unwrap();
    assert_eq!(u.code, UnresolvedCode::TimelineUnderfilled);
    assert_eq!(u.source_text, "30");
    assert_eq!(u.start_s.unwrap().millis(), 20_000);
}

#[test]
fn backwards_and_zero_intervals_are_rejected() {
    assert_eq!(
        kinds(en(&format!("{EN_HEAD}[8–3] AINO: Hi.\n"))),
        [IntakeErrorKind::InvalidInterval]
    );
    assert_eq!(
        kinds(en(&format!("{EN_HEAD}[3–3] AINO: Hi.\n"))),
        [IntakeErrorKind::InvalidInterval]
    );
}

#[test]
fn overlapping_dialogue_is_rejected_by_speaker_relation() {
    let same = format!("{EN_HEAD}[0–5] AINO: One.\n[4–8] AINO: Two.\n");
    assert_eq!(kinds(en(&same)), [IntakeErrorKind::OverlappingDialogue]);
    let other = format!("{EN_HEAD}[0–5] AINO: One.\n[4–8] RANE: Two.\n");
    let e = en(&other).unwrap_err();
    assert_eq!(e.errors[0].kind, IntakeErrorKind::OverlappingSpeakers);
    assert_eq!(
        e.errors[0].kind.category(),
        IntakeErrorCategory::Unsupported
    );
    // Action during speech is allowed (an insert may play under dialogue).
    let action = format!("{EN_HEAD}[0–30] AINO: One.\n[4–8] Rane looks at Aino.\n");
    assert!(en(&action).is_ok());
    let action = format!("{EN_HEAD}[0–4] Rane looks at Aino.\n[2–30] AINO: One.\n");
    assert!(en(&action).is_ok());
}

#[test]
fn out_of_order_lines_are_rejected() {
    let src = format!("{EN_HEAD}[5–8] AINO: One.\n[0–3] RANE: Two.\n");
    assert_eq!(kinds(en(&src)), [IntakeErrorKind::OutOfOrder]);
}

#[test]
fn unknown_speaker_is_not_added_as_a_character() {
    let src = format!("{EN_HEAD}[0–5] MIKKO: Hello.\n");
    let e = en(&src).unwrap_err();
    assert_eq!(e.errors[0].kind, IntakeErrorKind::UnknownSpeaker);
    assert_eq!(e.errors[0].span.unwrap().slice(&src), Some("MIKKO"));
    assert_eq!(
        e.errors[0].kind.category(),
        IntakeErrorCategory::InvalidReference
    );
}

#[test]
fn missing_colon_and_lowercase_label_are_reported_not_guessed() {
    let src = format!("{EN_HEAD}[0–5] RANE It's a tiny job.\n[5–9] Rane: Fine.\n");
    let e = en(&src).unwrap_err();
    assert_eq!(
        e.errors.iter().map(|x| x.kind).collect::<Vec<_>>(),
        [
            IntakeErrorKind::MissingColon,
            IntakeErrorKind::AmbiguousSpeaker
        ]
    );
    assert_eq!(e.errors[0].span.unwrap().slice(&src), Some("RANE"));
    assert_eq!(e.errors[1].span.unwrap().line_column(&src), (5, 7));
}

#[test]
fn empty_text_is_rejected() {
    assert_eq!(
        kinds(en(&format!("{EN_HEAD}[0–5] AINO:   \n"))),
        [IntakeErrorKind::EmptyText]
    );
    assert_eq!(
        kinds(en(&format!("{EN_HEAD}[0–5]\n"))),
        [IntakeErrorKind::EmptyText]
    );
}

#[test]
fn duration_mismatch_and_range_are_rejected() {
    assert_eq!(
        kinds(en(&format!("{EN_HEAD}[0–31] AINO: Too long.\n"))),
        [IntakeErrorKind::ExceedsDuration]
    );
    let short = EN_HEAD.replace("30 seconds", "29 seconds");
    assert_eq!(
        kinds(en(&format!("{short}[0–5] AINO: Hi.\n"))),
        [IntakeErrorKind::DurationOutOfRange]
    );
    let long = EN_HEAD.replace("30 seconds", "60.001 seconds");
    assert_eq!(
        kinds(en(&format!("{long}[0–5] AINO: Hi.\n"))),
        [IntakeErrorKind::DurationOutOfRange]
    );
    for bad in ["NaN", "inf", "-30", "3e1", "30.0001"] {
        let h = EN_HEAD.replace("30 seconds", &format!("{bad} seconds"));
        assert_eq!(
            kinds(en(&format!("{h}[0–5] AINO: Hi.\n"))),
            [IntakeErrorKind::InvalidTime],
            "{bad}"
        );
    }
    assert_eq!(
        kinds(en(&format!("{EN_HEAD}[0–NaN] AINO: Hi.\n"))),
        [IntakeErrorKind::InvalidTime]
    );
}

#[test]
fn header_errors() {
    assert_eq!(kinds(en("")), [IntakeErrorKind::MissingHeader]);
    let third = EN_HEAD.replace("Rane, boss.", "Rane, boss; Mikko, intern.");
    assert_eq!(
        kinds(en(&format!("{third}[0–5] AINO: Hi.\n"))),
        [IntakeErrorKind::TooManyCharacters]
    );
    let dup = EN_HEAD.replace("Rane, boss.", "Aino, boss.");
    assert_eq!(
        kinds(en(&format!("{dup}[0–5] AINO: Hi.\n"))),
        [IntakeErrorKind::DuplicateCharacter]
    );
    // Finnish text selected as English (and vice versa) is a language mismatch.
    assert_eq!(kinds(en(FI)), [IntakeErrorKind::LanguageMismatch]);
    assert_eq!(kinds(fi(EN)), [IntakeErrorKind::LanguageMismatch]);
    assert_eq!(kinds(en(EN_HEAD)), [IntakeErrorKind::NoTimedLines]);
}

#[test]
fn unsupported_lines_are_explicit() {
    let src = format!("{EN_HEAD}SCENE 2\n[0–5] AINO: Hi.\n  and more\n");
    assert_eq!(
        kinds(en(&src)),
        [
            IntakeErrorKind::UnsupportedSceneMarker,
            IntakeErrorKind::UnrecognizedLine
        ]
    );
}

#[test]
fn bounds_are_enforced() {
    let big = format!("{EN_HEAD}{}", "x".repeat(MAX_INTENT_SOURCE_BYTES));
    assert_eq!(kinds(en(&big)), [IntakeErrorKind::SourceTooLarge]);
    let mut many = EN_HEAD.to_string();
    for i in 0..65 {
        many.push_str(&format!(
            "[{}.{:03}–{}.{:03}] Tauko\n",
            i * 400 / 1000,
            i * 400 % 1000,
            (i * 400 + 400) / 1000,
            (i * 400 + 400) % 1000
        ));
    }
    assert!(kinds(en(&many)).contains(&IntakeErrorKind::TooManyBeats));
    let words = "a".repeat(501);
    let long = format!("{EN_HEAD}[0–5] AINO: {words}\n[5–10] RANE: {words}\n");
    assert_eq!(kinds(en(&long)), [IntakeErrorKind::DialogueTooLong]);
    let bad_utf8 = [EN_HEAD.as_bytes(), b"[0-5] AINO: \xff\n"].concat();
    let e = script_intent_from_text(&bad_utf8, &opts(IntentLanguage::EnGb)).unwrap_err();
    assert_eq!(e.errors[0].kind, IntakeErrorKind::InvalidUtf8);
    let mut o = opts(IntentLanguage::EnGb);
    o.scene_id = "Varikko".into();
    let e = script_intent_from_text(EN.as_bytes(), &o).unwrap_err();
    assert_eq!(e.errors[0].kind, IntakeErrorKind::InvalidOption);
}

#[test]
fn every_line_error_is_reported_together() {
    let src = format!("{EN_HEAD}[0–5] MIKKO: Hi.\n[5–4] AINO: Back.\n[6–7] AINO:\n");
    assert_eq!(
        kinds(en(&src)),
        [
            IntakeErrorKind::UnknownSpeaker,
            IntakeErrorKind::InvalidInterval,
            IntakeErrorKind::EmptyText
        ]
    );
}
