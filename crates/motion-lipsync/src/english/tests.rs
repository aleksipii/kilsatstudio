use super::*;
use crate::{AlignmentSource, Viseme};
use std::collections::BTreeMap;

fn words(t: &str) -> String {
    spoken_words(t, Lexicon::builtin()).join(" ")
}

fn none() -> Pronunciations {
    Pronunciations::new()
}

#[test]
fn numbers_ordinals_currency_percent() {
    assert_eq!(words("I have 3 cats"), "i have three cats");
    assert_eq!(words("1,250 items"), "one thousand two hundred fifty items");
    assert_eq!(words("3.14"), "three point one four");
    assert_eq!(words("the 21st and 2nd"), "the twenty first and second");
    assert_eq!(words("$5"), "five dollars");
    assert_eq!(words("$1"), "one dollar");
    assert_eq!(words("$1.50"), "one dollar fifty cents");
    assert_eq!(words("$5 million"), "five million dollars");
    assert_eq!(words("50%"), "fifty percent");
    assert_eq!(words("-5 degrees"), "minus five degrees");
    assert_eq!(words("007"), "zero zero seven");
    assert_eq!(words("10:30"), "ten thirty");
    assert_eq!(words("9:05"), "nine oh five");
    assert_eq!(words("5kg"), "five kilograms");
}

#[test]
fn years_and_decades() {
    assert_eq!(words("in 1999"), "in nineteen ninety nine");
    assert_eq!(words("2024"), "twenty twenty four");
    assert_eq!(words("2005"), "two thousand five");
    assert_eq!(words("1900"), "nineteen hundred");
    assert_eq!(words("the 1990s"), "the nineteen nineties");
    assert_eq!(words("4000"), "four thousand");
}

#[test]
fn abbreviations_symbols_casing_punctuation() {
    assert_eq!(
        words("Mr. Smith and Dr. Jones"),
        "mister smith and doctor jones"
    );
    assert_eq!(words("Rock & roll"), "rock and roll");
    assert_eq!(words("1 + 1 = 2"), "one plus one equals two");
    assert_eq!(words("email me @ noon"), "email me at noon");
    assert_eq!(words("e.g. this"), "for example this");
    assert_eq!(words("HELLO there"), "hello there");
    assert_eq!(words("ZQX"), "zee cue ex");
    assert_eq!(words("the U.S. team"), "the you ess team");
    assert_eq!(words("sooooo good"), "soo good");
    assert_eq!(words("well-known"), "well known");
    assert_eq!(words("don't stop, 'cause"), "don't stop cause");
    assert_eq!(words("Café"), "cafe");
    assert_eq!(words("OK"), "okay");
    assert_eq!(words("#1 fan"), "number one fan");
}

#[test]
fn pauses_merge_and_trim() {
    let t = normalize("...Wait!!! What?! Yes, ok.", Lexicon::builtin());
    assert!(matches!(t.first(), Some(Token::Word(_))));
    assert!(matches!(t.last(), Some(Token::Word(_))));
    let pauses = t.iter().filter(|t| matches!(t, Token::Pause(_))).count();
    assert_eq!(pauses, 3);
    assert!(normalize("?!...", Lexicon::builtin()).is_empty());
}

#[test]
fn lexicon_hits_and_is_compact() {
    let lex = Lexicon::builtin();
    assert!(lex.len() > 100_000);
    let hello: Vec<_> = lex
        .lookup("hello")
        .unwrap()
        .iter()
        .map(|p| p.symbol())
        .collect();
    assert_eq!(hello, ["HH", "AH", "L", "OW"]);
    assert!(lex.lookup("zzyzxqq").is_none());
    assert!(lex.contains("don't"));
    assert!(include_bytes!("../../data/cmudict.txt.gz").len() < 1_000_000);
}

#[test]
fn oov_fallback_is_deterministic_and_low_confidence() {
    let r1 = pronunciation_report("looksmaxxing is rizz", &none());
    let r2 = pronunciation_report("looksmaxxing is rizz", &none());
    assert_eq!(r1, r2);
    let w = &r1.words[0];
    assert_eq!(w.word, "looksmaxxing");
    assert_eq!(w.source, PronunciationSource::Fallback);
    assert!(w.low_confidence);
    assert_eq!(w.phonemes.join(" "), "L UH K S M AE K S IH NG");
    assert_eq!(r1.words[1].source, PronunciationSource::Lexicon);
    assert!(!r1.words[1].low_confidence);
    assert!(r1.low_confidence_words().contains(&"rizz"));
}

#[test]
fn fallback_never_empty_for_any_letter_string() {
    for w in [
        "a", "x", "qzx", "bcdfg", "aeiou", "eeee", "xyzzy", "ghgh", "mb", "e",
    ] {
        assert!(!letter_to_sound(w).is_empty(), "{w}");
    }
    assert!(letter_to_sound("").is_empty());
}

#[test]
fn derived_possessives_and_plurals() {
    let r = pronunciation_report("Zorblax's gadgets", &none());
    assert_eq!(r.words[0].source, PronunciationSource::Fallback);
    let r = pronunciation_report("keyboard's cats", &none());
    assert_eq!(r.words[0].source, PronunciationSource::Derived);
    assert!(!r.words[0].low_confidence);
    assert_eq!(r.words[0].phonemes.last(), Some(&"Z"));
}

fn overrides(pairs: &[(&str, &str)]) -> Result<Pronunciations, EnglishError> {
    let m: BTreeMap<String, String> = pairs
        .iter()
        .map(|(a, b)| (a.to_string(), b.to_string()))
        .collect();
    Pronunciations::from_map(&m)
}

#[test]
fn overrides_win_and_are_reported() {
    let o = overrides(&[("rizz", "R IH Z"), ("hello", "HH EH L OW")]).unwrap();
    let r = pronunciation_report("hello rizz world", &o);
    assert_eq!(r.words[0].source, PronunciationSource::Override);
    assert_eq!(r.words[0].phonemes, ["HH", "EH", "L", "OW"]);
    assert_eq!(r.words[1].source, PronunciationSource::Override);
    assert!(!r.words[1].low_confidence);
    assert_eq!(r.words[2].source, PronunciationSource::Lexicon);
}

#[test]
fn override_validation() {
    assert!(overrides(&[("ok", "K AE1 T")]).is_ok()); // stress digit tolerated
    assert!(overrides(&[("ok", "kat")]).is_err());
    assert!(overrides(&[("ok", "K ZZ")]).is_err());
    assert!(overrides(&[("ok", "")]).is_err());
    assert!(overrides(&[("Ok", "K")]).is_err());
    assert!(overrides(&[("", "K")]).is_err());
    assert!(overrides(&[("a b", "K")]).is_err());
    assert!(overrides(&[("x", &"K ".repeat(33))]).is_err());
    let big: BTreeMap<String, String> = (0..=MAX_OVERRIDES)
        .map(|i| {
            (
                format!("w{}", "a".repeat(1 + i % 5)) + &"b".repeat(i / 5 + 1),
                "K".into(),
            )
        })
        .collect();
    assert!(big.len() > MAX_OVERRIDES);
    assert!(Pronunciations::from_map(&big).is_err());
}

fn assert_track_ok(text: &str, frames: Option<u64>) -> EstimatedVisemeTrack {
    let t = estimate_viseme_track(text, &none(), frames).unwrap_or_else(|e| panic!("{text}: {e}"));
    assert!(!t.cues.is_empty(), "{text}");
    assert_eq!(t.cues[0].start_sample, 0);
    assert_eq!(t.cues.last().unwrap().end_sample, t.frame_count);
    if let Some(f) = frames {
        assert_eq!(t.frame_count, f);
    }
    for pair in t.cues.windows(2) {
        assert_eq!(pair[0].end_sample, pair[1].start_sample, "{text}");
    }
    let mut sum = 0;
    for c in &t.cues {
        assert!(c.end_sample > c.start_sample, "{text}");
        assert!(c.end_sample <= t.frame_count);
        sum += c.end_sample - c.start_sample;
    }
    assert_eq!(sum, t.frame_count, "{text}");
    assert_eq!(t.sample_rate, 24_000);
    t
}

#[test]
fn track_scales_to_audio_and_is_estimated() {
    let t = assert_track_ok("Hello there, how are you?", Some(24_000 * 3));
    match &t.source {
        AlignmentSource::Estimated { tool, version } => {
            assert_eq!(tool, TOOL_NAME);
            assert_eq!(version, TOOL_VERSION);
        }
        other => panic!("must be Estimated, got {other:?}"),
    }
    let unscaled = assert_track_ok("Hello there, how are you?", None);
    assert_eq!(unscaled.time_scale, 1.0);
    assert_eq!(unscaled.frame_count % 24, 0);
    assert!(t.time_scale > 1.0);
    // Deterministic.
    assert_eq!(
        t,
        assert_track_ok("Hello there, how are you?", Some(24_000 * 3))
    );
    // Lookup uses half-open intervals and rests outside.
    assert_eq!(t.viseme_at_sample(t.frame_count), Viseme::Rest);
    assert_eq!(t.viseme_at_sample(0), t.cues[0].viseme);
}

#[test]
fn track_visemes_follow_phonemes() {
    // "mom": M (Closed) AA (A) M (Closed)
    let t = assert_track_ok("mom", None);
    let v: Vec<_> = t.cues.iter().map(|c| c.viseme).collect();
    assert_eq!(v, [Viseme::Closed, Viseme::A, Viseme::Closed]);
    // Pause becomes rest.
    let t = assert_track_ok("mom. mom", None);
    assert!(t.cues.iter().any(|c| c.viseme == Viseme::Rest));
    // Velar "cook" borrows the vowel shape: K UH K.
    let t = assert_track_ok("cook", None);
    assert!(t.cues.iter().all(|c| c.viseme == Viseme::U));
}

#[test]
fn track_edge_cases() {
    assert_eq!(
        estimate_viseme_track("", &none(), None),
        Err(EnglishError::NoSpeakableText)
    );
    assert_eq!(
        estimate_viseme_track("?!... --", &none(), None),
        Err(EnglishError::NoSpeakableText)
    );
    assert!(matches!(
        estimate_viseme_track("hi", &none(), Some(0)),
        Err(EnglishError::InvalidFrameCount(0))
    ));
    assert!(estimate_viseme_track("hi", &none(), Some(MAX_TRACK_FRAMES + 1)).is_err());
    // Very short audio for long text still covers exactly and stays ordered.
    let t = assert_track_ok(
        "supercalifragilistic expialidocious extra words here",
        Some(5),
    );
    assert_eq!(t.frame_count, 5);
    // Huge input is bounded rather than unbounded.
    let long = "word ".repeat(100_000);
    assert!(estimate_viseme_track(&long, &none(), None).is_err());
    assert!(estimate_viseme_track(&long, &none(), Some(24_000)).is_ok());
}

#[test]
fn mouth_cue_conversion_preserves_intervals() {
    let t = assert_track_ok("see you soon", Some(24_000));
    let m = t.to_mouth_cues();
    assert_eq!(m.len(), t.cues.len());
    for (a, b) in t.cues.iter().zip(&m) {
        assert_eq!(
            (a.start_sample, a.end_sample),
            (b.start_sample, b.end_sample)
        );
        assert!(crate::rhubarb::shape_viseme(b.shape).is_some());
    }
}

const SENTENCES: [&str; 60] = [
    "Hello, how are you today?",
    "The quick brown fox jumps over the lazy dog.",
    "I can't believe it's already 2025!",
    "She sells sea shells by the sea shore.",
    "Meet me at 10:30 on the 3rd floor.",
    "That costs $4.99, plus 8% tax.",
    "Dr. Smith said, \"Wait here, please.\"",
    "What on earth are you talking about?!",
    "We're going to the zoo, and then we'll eat lunch.",
    "Looksmaxxing is the new trend, apparently.",
    "He whispered: nobody knows the answer...",
    "Wow!!! That was absolutely incredible!!!",
    "Please call 555 0199 before nine.",
    "My favourite colour is turquoise.",
    "Rizz, sigma, skibidi, and other strange words.",
    "The year 1984 was a long time ago; 2001 was later.",
    "Thirty-three thousand people watched the game.",
    "Can you hear the rhythm of the rain?",
    "Beware of the thorough, thoughtful, thankful thief.",
    "Xylophones and zebras live in imaginary zoos.",
    "NASA and the FBI aren't involved, I promise.",
    "It's 5 o'clock somewhere, my friend.",
    "Mr. and Mrs. Jones live at number 42.",
    "Rock & roll will never die!",
    "Is 2 + 2 = 4? Yes, of course.",
    "The knight knew the knot was tight.",
    "A psychologist wrote about phlegm and pneumonia.",
    "Quietly, she queued behind the quarrelsome queen.",
    "Pack my box with five dozen liquor jugs.",
    "How vexingly quick daft zebras jump!",
    "Oh no, not again... please, no.",
    "Sooooo good, soooo very good.",
    "The CEO of Acme Corp. earned $2.5 million in 2023.",
    "Tomorrow's weather: sunny, with a high of 75 degrees.",
    "Zorblax the Magnificent conquered Qwertyland.",
    "Let's go! We're late for the 1st act.",
    "Hmm, interesting. Tell me more.",
    "Nobody expects the Spanish Inquisition.",
    "She said she'd be there by 6:45.",
    "One, two, three, four, five, six, seven, eight, nine, ten.",
    "I'm 100% sure that it won't rain.",
    "Gigantic jellyfish glided through the gentle current.",
    "The cat's whiskers twitched as dogs' barks echoed.",
    "Whoa, that's a lot of cheese!",
    "Beautiful, wonderful, magnificent, extraordinary.",
    "Once upon a time, in a land far, far away...",
    "Run! Hide! They're coming!",
    "Use the e.g. rule, i.e. the simple one.",
    "Café au lait, naïve façade, and jalapeño peppers.",
    "Hello???",
    "x",
    "I",
    "A",
    "Yes.",
    "ABCDEFG HIJKLMNOP",
    "supercalifragilisticexpialidocious",
    "Don't you dare, y'all.",
    "Take 2 tablets, 3 times a day, for 14 days.",
    "Temperature dropped to -5 degrees overnight.",
    "Ladies and gentlemen, welcome to the show!",
];

#[test]
fn sixty_sentences_never_panic_or_return_empty() {
    assert!(SENTENCES.len() >= 50);
    let lex = Lexicon::builtin();
    for text in SENTENCES {
        let report = pronunciation_report(text, &none());
        assert!(!report.words.is_empty(), "{text}");
        for w in &report.words {
            assert!(!w.phonemes.is_empty(), "{text}: {}", w.word);
            assert!(w.word.bytes().all(|b| b.is_ascii_lowercase() || b == b'\''));
        }
        assert!(!spoken_words(text, lex).is_empty());
        assert_track_ok(text, None);
        assert_track_ok(text, Some(24_000 * 2));
    }
}

#[test]
fn arbitrary_bytes_do_not_panic() {
    let junk = [
        "\u{0}\u{1}\u{7f}",
        "💥🎉 日本語 текст",
        "$$$$ 99999999999999999999999 %%% ,,,, ....",
        "1,2,3,,,4 5..6 7.",
        "'''' ''a'' a'' ''",
        "-----5",
        "9999999999999999999999999.9999999999",
        "e.g.e.g.i.e. U.S.A.",
    ];
    for j in junk {
        let _ = pronunciation_report(j, &none());
        let _ = estimate_viseme_track(j, &none(), None);
    }
}

#[test]
fn span_estimate_rests_outside_speech_and_matches_shifted_estimate() {
    // "mom" = M AA M; speech in [6000, 12000) of an 18000-sample clip.
    let t = estimate_viseme_track_in_span("mom", &none(), 18000, 6000, 12000).unwrap();
    let inner = estimate_viseme_track("mom", &none(), Some(6000)).unwrap();
    assert_eq!(t.frame_count, 18000);
    assert_eq!(t.source, inner.source);
    assert_eq!(t.time_scale, inner.time_scale);
    assert_eq!(t.cues.first().unwrap().viseme, Viseme::Rest);
    assert_eq!(t.cues.first().unwrap().end_sample, 6000);
    assert_eq!(t.cues.last().unwrap().viseme, Viseme::Rest);
    assert_eq!(t.cues.last().unwrap().start_sample, 12000);
    assert_eq!(t.cues.last().unwrap().end_sample, 18000);
    for s in (0..18000).step_by(97) {
        let want = if (6000..12000).contains(&s) {
            inner.viseme_at_sample(s - 6000)
        } else {
            Viseme::Rest
        };
        assert_eq!(t.viseme_at_sample(s), want, "sample {s}");
    }
    // Contiguous cover and determinism.
    assert!(t
        .cues
        .windows(2)
        .all(|w| w[0].end_sample == w[1].start_sample && w[0].viseme != w[1].viseme));
    assert_eq!(
        t,
        estimate_viseme_track_in_span("mom", &none(), 18000, 6000, 12000).unwrap()
    );
}

#[test]
fn full_span_equals_whole_clip_estimate_and_identity() {
    let text = "Oh OK. Is that a game?";
    assert_eq!(
        estimate_viseme_track_in_span(text, &none(), 24000, 0, 24000).unwrap(),
        estimate_viseme_track(text, &none(), Some(24000)).unwrap()
    );
    assert_eq!(
        estimate_identity_in_span(text, &none(), 24000, 0, 24000),
        estimate_identity(text, &none(), 24000)
    );
    let a = estimate_identity_in_span(text, &none(), 24000, 2400, 20000);
    assert_ne!(a, estimate_identity(text, &none(), 24000));
    assert_ne!(
        a,
        estimate_identity_in_span(text, &none(), 24000, 2400, 20001)
    );
    assert_eq!(
        a,
        estimate_identity_in_span(text, &none(), 24000, 2400, 20000)
    );
}

#[test]
fn span_estimate_rejects_invalid_spans() {
    for (frames, s, e) in [(0, 0, 0), (100, 50, 50), (100, 60, 50), (100, 0, 101)] {
        assert!(estimate_viseme_track_in_span("hi", &none(), frames, s, e).is_err());
    }
    assert_eq!(
        estimate_viseme_track_in_span("...", &none(), 100, 0, 100),
        Err(EnglishError::NoSpeakableText)
    );
}
