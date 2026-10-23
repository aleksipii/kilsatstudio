//! Text-estimated mouths in the dialogue pipeline: precedence, exact samples, scaling,
//! trim/offset/seek, cache identity and diagnostics. Structural checks only; they say
//! nothing about perceptual lip-sync quality. WAV fixtures are generated at test time.
use motion_audio::{write_wav, AudioBuffer};
use motion_lipsync::english::{estimate_identity, estimate_viseme_track, Pronunciations};
use motion_lipsync::rhubarb::{EstimatedVisemeAlignment, MouthCue, Recognizer};
use motion_lipsync::{Alignment, AlignmentSource, TimedPhoneme, Viseme};
use motion_tts::{SpeechError, SpeechProvider, SpeechRequest, SpeechResult};
use motion_voice::diagnostics::PreparationDiagnostic;
use motion_voice::scene_audio::prepare_scene;
use motion_voice::timeline::{DialogueClip, MouthSource};
use std::collections::BTreeMap;

fn clip(frames: usize, start_frame: u64, trim: u64) -> DialogueClip {
    DialogueClip {
        actor: "PUPPET".into(),
        start_frame,
        audio: AudioBuffer {
            sample_rate: 24000,
            channels: 1,
            samples: vec![0.25; frames],
        },
        alignment: None,
        estimated_alignment: None,
        text_estimated: None,
        gain: 1.0,
        trim_start_frame: trim,
    }
}
fn none() -> Pronunciations {
    Pronunciations::new()
}
fn overrides(pairs: &[(&str, &str)]) -> Pronunciations {
    Pronunciations::from_map(
        &pairs
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect::<BTreeMap<_, _>>(),
    )
    .unwrap()
}
fn manual() -> Alignment {
    Alignment {
        source: AlignmentSource::Manual,
        duration_seconds: 0.5,
        phonemes: vec![TimedPhoneme {
            phoneme: "u".into(),
            start_seconds: 0.0,
            end_seconds: 0.25,
        }],
    }
}
fn rhubarb(frames: u64) -> EstimatedVisemeAlignment {
    EstimatedVisemeAlignment {
        schema_version: 1,
        source: "estimated_rhubarb".into(),
        tool_version: "1".into(),
        executable_sha256: "0".repeat(64),
        pcm_sha256: "0".repeat(64),
        transcript_sha256: "0".repeat(64),
        recognizer: Recognizer::Phonetic,
        sample_rate: 24000,
        frame_count: frames,
        cues: vec![MouthCue {
            start_sample: 0,
            end_sample: frames,
            shape: 'E', // Viseme::O
        }],
    }
}

/// "mom" is M AA M = 70 + 110 + 70 ms. Unscaled (6000 frames): Closed 0..1680,
/// A 1680..4320, Closed 4320..6000.
#[test]
fn exact_samples_at_known_frames() {
    let mut c = clip(6000, 0, 0);
    let info = c.derive_text_estimate("mom", &none()).unwrap().unwrap();
    assert!(info.low_confidence_words.is_empty());
    assert!((info.time_scale - 1.0).abs() < 1e-9);
    assert_eq!(c.mouth_source(), MouthSource::TextEstimated);
    for (sample, want) in [
        (0, Viseme::Closed),
        (1679, Viseme::Closed),
        (1680, Viseme::A),
        (4319, Viseme::A),
        (4320, Viseme::Closed),
        (5999, Viseme::Closed),
        (6000, Viseme::Rest),
    ] {
        assert_eq!(c.mouth_at(sample).unwrap(), want, "sample {sample}");
    }
}

#[test]
fn track_is_scaled_exactly_to_the_audio_frames() {
    let mut c = clip(12000, 0, 0);
    let info = c.derive_text_estimate("mom", &none()).unwrap().unwrap();
    assert!((info.time_scale - 2.0).abs() < 1e-9);
    let track = &c.text_estimated.as_ref().unwrap().track;
    assert_eq!(track.frame_count, 12000);
    assert_eq!(track.cues.first().unwrap().start_sample, 0);
    assert_eq!(track.cues.last().unwrap().end_sample, 12000);
    assert_eq!(
        track.source,
        AlignmentSource::Estimated {
            tool: "kilsat-english-estimator".into(),
            version: "1".into()
        }
    );
    assert_eq!(c.mouth_at(3359).unwrap(), Viseme::Closed);
    assert_eq!(c.mouth_at(3360).unwrap(), Viseme::A);
    assert_eq!(c.mouth_at(8639).unwrap(), Viseme::A);
    assert_eq!(c.mouth_at(8640).unwrap(), Viseme::Closed);
    assert_eq!(c.mouth_at(12000).unwrap(), Viseme::Rest);
}

#[test]
fn offset_and_trim_use_the_shared_sample_clock() {
    let mut c = clip(6000, 1000, 0);
    c.derive_text_estimate("mom", &none()).unwrap();
    assert_eq!(c.mouth_at(999).unwrap(), Viseme::Rest);
    assert_eq!(c.mouth_at(1000).unwrap(), Viseme::Closed);
    assert_eq!(c.mouth_at(1000 + 1680).unwrap(), Viseme::A);
    assert_eq!(c.mouth_at(1000 + 6000).unwrap(), Viseme::Rest);
    // Trimming the first 1680 source frames starts the clip on the A shape.
    let mut t = clip(6000, 1000, 1680);
    t.derive_text_estimate("mom", &none()).unwrap();
    assert_eq!(t.mouth_at(1000).unwrap(), Viseme::A);
    assert_eq!(t.mouth_at(1000 + 4320 - 1680).unwrap(), Viseme::Closed);
    assert_eq!(t.mouth_at(1000 + 6000 - 1680).unwrap(), Viseme::Rest);
}

#[test]
fn seeking_out_of_order_gives_the_same_mouth() {
    let mut c = clip(9600, 480, 0);
    c.derive_text_estimate("hello there my friend", &none())
        .unwrap();
    let frames: Vec<u64> = (0..10_200).step_by(37).collect();
    let forward: Vec<_> = frames.iter().map(|f| c.mouth_at(*f).unwrap()).collect();
    let mut shuffled = frames.clone();
    // Deterministic permutation: reverse then interleave halves.
    shuffled.reverse();
    let (a, b) = shuffled.split_at(shuffled.len() / 2);
    let order: Vec<u64> = a.iter().zip(b).flat_map(|(x, y)| [*x, *y]).collect();
    for f in order {
        let i = frames.iter().position(|x| *x == f).unwrap();
        assert_eq!(c.mouth_at(f).unwrap(), forward[i], "frame {f}");
    }
    assert!(forward.iter().any(|v| *v != Viseme::Rest));
}

#[test]
fn precedence_manual_forced_rhubarb_text() {
    let text = "mom";
    // Text only.
    let mut c = clip(12000, 0, 0);
    c.derive_text_estimate(text, &none()).unwrap();
    assert_eq!(c.mouth_source(), MouthSource::TextEstimated);
    // Rhubarb estimated outranks text and removes the derived track.
    c.estimated_alignment = Some(rhubarb(12000));
    assert!(c.derive_text_estimate(text, &none()).unwrap().is_none());
    assert!(c.text_estimated.is_none());
    assert_eq!(c.mouth_source(), MouthSource::RhubarbEstimated);
    assert_eq!(c.mouth_at(100).unwrap(), Viseme::O);
    // Forced and manual outrank both.
    let mut f = clip(12000, 0, 0);
    f.alignment = Some(Alignment {
        source: AlignmentSource::Forced {
            tool: "t".into(),
            version: "1".into(),
        },
        ..manual()
    });
    assert!(f.derive_text_estimate(text, &none()).unwrap().is_none());
    assert_eq!(f.mouth_source(), MouthSource::Forced);
    let mut m = clip(12000, 0, 0);
    m.alignment = Some(manual());
    m.derive_text_estimate(text, &none()).unwrap();
    assert_eq!(m.mouth_source(), MouthSource::Manual);
    assert_eq!(m.mouth_at(0).unwrap(), Viseme::U);
    assert_eq!(MouthSource::None, clip(10, 0, 0).mouth_source());
}

#[test]
fn identity_covers_text_overrides_frames_and_lexicon_version() {
    let base = estimate_identity("zxqvbn rocks", &none(), 6000);
    assert_eq!(base, estimate_identity("zxqvbn rocks", &none(), 6000));
    let a = overrides(&[("zxqvbn", "Z AH K")]);
    let b = overrides(&[("zxqvbn", "Z AH K S")]);
    let ia = estimate_identity("zxqvbn rocks", &a, 6000);
    assert_ne!(base, ia);
    assert_ne!(ia, estimate_identity("zxqvbn rocks", &b, 6000));
    assert_ne!(base, estimate_identity("zxqvbn rock", &none(), 6000));
    assert_ne!(base, estimate_identity("zxqvbn rocks", &none(), 6001));
    assert!(motion_lipsync::english::lexicon_version().starts_with("kilsat-english-estimator-1-"));
}

#[test]
fn changing_an_override_invalidates_the_stored_estimate() {
    let text = "zxqvbn";
    let a = overrides(&[("zxqvbn", "P AA P")]);
    let b = overrides(&[("zxqvbn", "UW")]);
    let mut c = clip(6000, 0, 0);
    c.derive_text_estimate(text, &a).unwrap();
    let first = c.text_estimated.clone().unwrap();
    // Same inputs keep the identical estimate.
    c.derive_text_estimate(text, &a).unwrap();
    assert_eq!(c.text_estimated.as_ref().unwrap(), &first);
    // A changed override replaces it with a different track and identity.
    c.derive_text_estimate(text, &b).unwrap();
    let second = c.text_estimated.clone().unwrap();
    assert_ne!(first.identity, second.identity);
    assert_ne!(first.track.cues, second.track.cues);
    assert_eq!(
        second.track.cues,
        estimate_viseme_track(text, &b, Some(6000)).unwrap().cues
    );
}

#[test]
fn low_confidence_words_are_reported_and_overrides_silence_them() {
    let mut c = clip(6000, 0, 0);
    let info = c
        .derive_text_estimate("zxqvbn is mom", &none())
        .unwrap()
        .unwrap();
    assert_eq!(info.low_confidence_words, ["zxqvbn"]);
    let o = overrides(&[("zxqvbn", "Z AH K")]);
    let info = c
        .derive_text_estimate("zxqvbn is mom", &o)
        .unwrap()
        .unwrap();
    assert!(info.low_confidence_words.is_empty());
}

struct Wav(std::path::PathBuf, usize);
impl Wav {
    fn new(name: &str, frames: usize) -> Self {
        let path = std::env::temp_dir().join(format!(
            "kilsat-textviseme-{}-{name}.wav",
            std::process::id()
        ));
        write_wav(
            &path,
            &AudioBuffer {
                sample_rate: 24000,
                channels: 1,
                samples: vec![0.25; frames],
            },
        )
        .unwrap();
        Self(path, frames)
    }
}
impl Drop for Wav {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}
impl SpeechProvider for Wav {
    fn synthesize(&self, _: SpeechRequest) -> Result<SpeechResult, SpeechError> {
        Ok(SpeechResult {
            audio_path: self.0.clone(),
            sample_rate: 24000,
            sample_count: self.1 as u64,
            cache_key: String::new(),
            phonemes: String::new(),
            cached: false,
        })
    }
}
fn script(pron: &str, text: &str) -> motion_script::Script {
    motion_script::parse(&format!(
        "{pron}characters:\n  EMMA: {{asset: characters/emma, voice: {{provider: kokoro, preset: af_heart, language: en-us, speed: 1.0}}}}\nscenes:\n  - id: s\n    duration: 1.0\n    events:\n      - {{type: speak, at: 0.5, actor: EMMA, text: \"{text}\"}}\n"
    ))
    .unwrap()
}

#[test]
fn prepared_scene_has_text_mouths_diagnostics_and_manifest_matches_mouth_at() {
    let provider = Wav::new("scene", 6000);
    let root = std::path::Path::new(".");
    let prepared = prepare_scene(
        &script("", "zxqvbn mom"),
        0,
        root,
        &provider,
        &BTreeMap::new(),
        |_| {},
    )
    .unwrap();
    assert_eq!(prepared.clips[0].mouth_source(), MouthSource::TextEstimated);
    // Poor-fit diagnostics may accompany it; the line under test is the low-confidence one.
    let low: Vec<_> = prepared
        .diagnostics
        .iter()
        .filter(|d| matches!(d, PreparationDiagnostic::LowConfidencePronunciation { .. }))
        .collect();
    assert_eq!(low.len(), 1);
    assert!(matches!(
        low[0],
        PreparationDiagnostic::LowConfidencePronunciation { dialogue_id: 0, words, .. } if words == &["zxqvbn"]
    ));
    assert!(low[0].message().contains("pronunciations:"));
    let manifest = &prepared.rendered.manifest.dialogue[0];
    assert_eq!(
        manifest.alignment_source,
        Some(AlignmentSource::Estimated {
            tool: "kilsat-english-estimator".into(),
            version: "1".into()
        })
    );
    // Preview (mouth_at) and export (manifest intervals) agree at every scene sample.
    for sample in 0..24000u64 {
        let from_manifest = manifest
            .visemes
            .iter()
            .find(|v| v.start_frame <= sample && sample < v.end_frame)
            .map_or(Viseme::Rest, |v| v.viseme);
        let in_clip = sample >= manifest.start_frame
            && sample < manifest.start_frame + manifest.duration_frames;
        let want = if in_clip {
            prepared.clips[0].mouth_at(sample).unwrap()
        } else {
            Viseme::Rest
        };
        assert_eq!(from_manifest, want, "sample {sample}");
    }
    // An override removes the diagnostic and changes the mouth data.
    let pron = "pronunciations:\n  zxqvbn: \"Z AH K\"\n";
    let with = prepare_scene(
        &script(pron, "zxqvbn mom"),
        0,
        root,
        &provider,
        &BTreeMap::new(),
        |_| {},
    )
    .unwrap();
    assert!(!with
        .diagnostics
        .iter()
        .any(|d| matches!(d, PreparationDiagnostic::LowConfidencePronunciation { .. })));
    assert_ne!(with.rendered.manifest.dialogue[0].visemes, manifest.visemes);
}

#[test]
fn explicit_alignment_wins_and_gets_no_text_diagnostics() {
    let provider = Wav::new("manual", 12000);
    let a = Alignment {
        duration_seconds: 0.5,
        ..manual()
    };
    let prepared = prepare_scene(
        &script("", "zxqvbn"),
        0,
        std::path::Path::new("."),
        &provider,
        &BTreeMap::from([(0, a)]),
        |_| {},
    )
    .unwrap();
    assert_eq!(prepared.clips[0].mouth_source(), MouthSource::Manual);
    assert!(prepared.diagnostics.is_empty());
    assert_eq!(
        prepared.rendered.manifest.dialogue[0].alignment_source,
        Some(AlignmentSource::Manual)
    );
}

#[test]
fn unspeakable_text_is_a_non_fatal_diagnostic_and_the_mouth_rests() {
    let provider = Wav::new("silent", 6000);
    let prepared = prepare_scene(
        &script("", "..."),
        0,
        std::path::Path::new("."),
        &provider,
        &BTreeMap::new(),
        |_| {},
    )
    .unwrap();
    assert!(matches!(
        prepared.diagnostics[0],
        PreparationDiagnostic::TextEstimateUnavailable { .. }
    ));
    assert_eq!(prepared.clips[0].mouth_source(), MouthSource::None);
    assert_eq!(prepared.clips[0].mouth_at(12000).unwrap(), Viseme::Rest);
}

/// Tone with digital-silence padding (generated, not speech).
fn padded(lead: usize, speech: usize, trail: usize) -> AudioBuffer {
    let mut samples = vec![0.0f32; lead];
    samples.extend((0..speech).map(|i| 0.3 * (i as f32 * 0.05).sin()));
    samples.extend(vec![0.0f32; trail]);
    AudioBuffer {
        sample_rate: 24000,
        channels: 1,
        samples,
    }
}

#[test]
fn text_estimate_rests_over_leading_and_trailing_silence() {
    // 0.4 s silence, 1 s tone, 0.6 s silence: the shape seen in the real Kokoro clips.
    let mut c = clip(1, 0, 0);
    c.audio = padded(9600, 24000, 14400);
    let (start, end) = motion_voice::timeline::speech_span(&c.audio);
    // The 20 ms centred window spreads at most one 10 ms hop into silence.
    assert!(start.abs_diff(9600) <= 240 && end.abs_diff(33600) <= 240);
    let info = c.derive_text_estimate("mom", &none()).unwrap().unwrap();
    let track = &c.text_estimated.as_ref().unwrap().track;
    assert_eq!(track.frame_count, 48000);
    assert!((info.time_scale - (end - start) as f64 / 6000.0).abs() < 1e-9);
    for s in [0, start - 1, end, 47999] {
        assert_eq!(c.mouth_at(s).unwrap(), Viseme::Rest, "sample {s}");
    }
    // Inside the span the mapping of "mom" is shifted: closed, open, closed.
    assert_eq!(c.mouth_at(start).unwrap(), Viseme::Closed);
    assert_eq!(c.mouth_at((start + end) / 2).unwrap(), Viseme::A);
    assert_eq!(c.mouth_at(end - 1).unwrap(), Viseme::Closed);
    // Same length, different silence: identity changes and the estimate is rebuilt.
    let old = c.text_estimated.as_ref().unwrap().identity.clone();
    c.audio = padded(4800, 24000, 19200);
    c.derive_text_estimate("mom", &none()).unwrap();
    assert_ne!(old, c.text_estimated.as_ref().unwrap().identity);
    assert_eq!(c.mouth_at(9000).unwrap(), Viseme::Closed);
    assert_eq!(c.mouth_at(4000).unwrap(), Viseme::Rest);
}

#[test]
fn silent_clip_keeps_whole_clip_estimate() {
    let mut c = clip(1, 0, 0);
    c.audio = padded(6000, 0, 0);
    assert_eq!(motion_voice::timeline::speech_span(&c.audio), (0, 6000));
    c.derive_text_estimate("mom", &none()).unwrap();
    let estimate = c.text_estimated.as_ref().unwrap();
    assert_eq!(estimate.identity, estimate_identity("mom", &none(), 6000));
    assert_eq!(
        estimate.track,
        estimate_viseme_track("mom", &none(), Some(6000)).unwrap()
    );
}
