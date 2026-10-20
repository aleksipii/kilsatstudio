//! Acting layer through the shared SVG path: PNG pixels and preview frames agree.
use super::*;
use std::sync::Arc;

const ACTING: &str = include_str!("../../../examples/acting-rig.yaml");

fn script() -> Script {
    motion_script::parse(ACTING).unwrap()
}
/// SVG, rasterized, for the scene at `time` with the optional PUPPET viseme.
fn raster(script: &Script, time: f64, viseme: Option<Viseme>) -> resvg::tiny_skia::Pixmap {
    let mut scene = motion_scene::evaluate(script, 0, time).unwrap();
    let mouths: Vec<(&str, Viseme)> = viseme.map(|v| ("PUPPET", v)).into_iter().collect();
    motion_scene::apply_speech(&mut scene, &mouths);
    let svg = motion_render::render_svg_with_art(
        &scene,
        256,
        256,
        &mouths,
        "acting",
        &motion_art::Document::default(),
        &BTreeMap::new(),
    )
    .unwrap();
    resvg::tiny_skia::Pixmap::decode_png(&media::rasterize(&svg).unwrap()).unwrap()
}
/// Pixel at an offset from the puppet's origin (128, 179.2 on a 256 canvas).
fn at(p: &resvg::tiny_skia::Pixmap, dx: f64, dy: f64) -> [u8; 3] {
    let c = p
        .pixel((128.0 + dx).floor() as u32, (179.2 + dy).floor() as u32)
        .unwrap();
    [c.red(), c.green(), c.blue()]
}
const FACE: [u8; 3] = [240, 200, 160];
const MOUTH: [u8; 3] = [120, 40, 50];
const WHITE: [u8; 3] = [255, 255, 255];
const PUPIL: [u8; 3] = [20, 20, 40];

fn blink_time(script: &Script, blinking: bool) -> f64 {
    let rig = script.characters["PUPPET"]
        .rig
        .as_ref()
        .unwrap()
        .prepare()
        .unwrap();
    (0..48)
        .map(|i| f64::from(i) / 24.0)
        .find(|t| rig.blink_active("PUPPET", *t) == blinking)
        .unwrap()
}

#[test]
fn viseme_variant_pixels_and_preview_agree_with_the_svg_png_path() {
    let script = script();
    // Head joint at y = -40; the base mouth spans y -26..-23, the open variant -30..-14.
    let closed = raster(&script, 1.0, None);
    assert_eq!(at(&closed, 0.0, -24.5), MOUTH);
    assert_eq!(at(&closed, 0.0, -18.0), FACE);
    let open = raster(&script, 1.0, Some(Viseme::A));
    assert_eq!(at(&open, 0.0, -18.0), MOUTH);
    assert_eq!(at(&open, 0.0, -28.0), MOUTH);
    let round = raster(&script, 1.0, Some(Viseme::O));
    assert_eq!(at(&round, 8.0, -22.0), FACE);
    assert_eq!(at(&round, 0.0, -22.0), MOUTH);
    // The preview path (selected-line playback override) renders the identical frame.
    let preview = ScenePreview::new(
        Arc::new(script.clone()),
        0,
        Arc::new(ArtContext::default()),
        None,
    )
    .unwrap();
    let cancel = CancellationToken::new();
    for (sample, viseme) in [(24_000u64, Viseme::A), (0, Viseme::O), (24_000, Viseme::A)] {
        let frame = preview
            .render_with_mouths(
                sample,
                256,
                Some(&[("PUPPET".to_string(), viseme)]),
                &cancel,
            )
            .unwrap();
        let expected = raster(&script, sample as f64 / 24_000.0, Some(viseme));
        assert_eq!(frame.rgba, expected.data(), "sample {sample}");
    }
    let frame = preview.render(24_000, 256, &cancel).unwrap();
    assert_eq!(frame.rgba, raster(&script, 1.0, None).data());
}

#[test]
fn blink_and_gaze_pixels_follow_the_evaluated_scene() {
    let script = script();
    let open = blink_time(&script, false);
    let shut = blink_time(&script, true);
    // Left eye white spans x -22..-8, y -54..-40 from the origin; pupil sits at x -17..-13.
    let awake = raster(&script, open, None);
    assert_eq!(at(&awake, -20.0, -52.0), WHITE);
    let blink = raster(&script, shut, None);
    assert_eq!(at(&blink, -20.0, -52.0), FACE);
    assert_eq!(at(&blink, -15.0, -47.0), [60, 40, 40]);
    // Gaze at t = 2 s is full right: the pupil moved 4 units from x -17..-13 to -13..-9,
    // so x -10 is pupil and x -19 is white. At t = 0 it is full left.
    let find = |gaze_time: f64| {
        let mut t = gaze_time;
        while blink_time_at(&script, t) {
            t += 1.0 / 24.0;
        }
        t
    };
    let right = raster(&script, find(2.0), None);
    assert_eq!(at(&right, -10.5, -47.0), PUPIL);
    assert_eq!(at(&right, -19.0, -47.0), WHITE);
    let left = raster(&script, find(0.0), None);
    assert_eq!(at(&left, -19.5, -47.0), PUPIL);
    assert_eq!(at(&left, -10.5, -47.0), WHITE);
    // Seeking back and forth is bit-identical.
    for time in [find(2.0), find(0.0), find(2.0)] {
        let again = raster(&script, time, None);
        let want = if time < 1.0 { &left } else { &right };
        assert_eq!(again.data(), want.data());
    }
}

fn blink_time_at(script: &Script, t: f64) -> bool {
    script.characters["PUPPET"]
        .rig
        .as_ref()
        .unwrap()
        .prepare()
        .unwrap()
        .blink_active("PUPPET", t)
}

fn dialogue_clip(actor: &str, start: u64, frames: usize) -> motion_voice::timeline::DialogueClip {
    motion_voice::timeline::DialogueClip {
        actor: actor.into(),
        start_frame: start,
        audio: motion_audio::AudioBuffer {
            sample_rate: 24_000,
            channels: 1,
            samples: vec![0.25; frames],
        },
        alignment: None,
        estimated_alignment: None,
        text_estimated: None,
        gain: 1.0,
        trim_start_frame: 0,
    }
}
fn mouth_pixel_open(script: &Script, rendered: &RenderedScene, frame: u64) -> (Viseme, bool) {
    let sample = frame * 1000; // 24 fps on the 24 kHz clock
    let mouths = mouths_at(&rendered.manifest, sample);
    let viseme = mouths
        .iter()
        .find(|(actor, _)| *actor == "PUPPET")
        .map(|(_, v)| *v);
    let px = raster(script, sample as f64 / 24_000.0, viseme);
    (viseme.unwrap_or(Viseme::Rest), at(&px, 0.0, -18.0) == MOUTH)
}

/// Text-estimated mouths, scaled to a 0.5 s clip starting at 1.0 s: "mom" is
/// Closed 0..3360, A 3360..8640, Closed 8640..12000 clip samples. Only A maps to the
/// rig's `open` mouth variant, so the open mouth appears for scene samples
/// 27360..32640, i.e. frames 28 to 32 at 24 fps.
#[test]
fn text_estimated_viseme_changes_the_rendered_mouth_at_the_expected_sample() {
    let script = script();
    let mut clip = dialogue_clip("PUPPET", 24_000, 12_000);
    clip.derive_text_estimate("mom", &motion_lipsync::english::Pronunciations::new())
        .unwrap();
    let rendered = motion_voice::export::render_scene("acting", 6.0, &[clip]).unwrap();
    assert_eq!(
        rendered.manifest.dialogue[0].alignment_source,
        Some(AlignmentSource::Estimated {
            tool: "kilsat-english-estimator".into(),
            version: "1".into()
        })
    );
    for frame in 0..=40u64 {
        let (viseme, open) = mouth_pixel_open(&script, &rendered, frame);
        let expect_open = (28..=32).contains(&frame);
        assert_eq!(open, expect_open, "frame {frame} viseme {viseme:?}");
        assert_eq!(viseme == Viseme::A, expect_open, "frame {frame}");
    }
    // Sample-exact boundary in the manifest the exporter reads.
    let at_sample = |s: u64| {
        mouths_at(&rendered.manifest, s)
            .into_iter()
            .find(|(a, _)| *a == "PUPPET")
            .map(|(_, v)| v)
    };
    assert_eq!(at_sample(24_000 + 3359), Some(Viseme::Closed));
    assert_eq!(at_sample(24_000 + 3360), Some(Viseme::A));
    assert_eq!(at_sample(24_000 + 12_000), None);
}

/// Without text data the rig renders exactly as before (base mouth throughout), and an
/// explicit manual alignment is untouched by the text-estimate step.
#[test]
fn rigs_without_estimates_or_with_manual_alignment_are_unchanged() {
    let script = script();
    let silent = dialogue_clip("PUPPET", 24_000, 12_000);
    let rendered = motion_voice::export::render_scene("acting", 6.0, &[silent]).unwrap();
    assert_eq!(rendered.manifest.dialogue[0].alignment_source, None);
    for frame in 24..=36u64 {
        let (viseme, open) = mouth_pixel_open(&script, &rendered, frame);
        assert_eq!((viseme, open), (Viseme::Rest, false), "frame {frame}");
    }
    let manual = motion_lipsync::Alignment {
        source: AlignmentSource::Manual,
        duration_seconds: 0.5,
        phonemes: vec![motion_lipsync::TimedPhoneme {
            phoneme: "a".into(),
            start_seconds: 0.0,
            end_seconds: 0.25,
        }],
    };
    let build = |derive: bool| {
        let mut clip = dialogue_clip("PUPPET", 24_000, 12_000);
        clip.alignment = Some(manual.clone());
        if derive {
            clip.derive_text_estimate("mom", &motion_lipsync::english::Pronunciations::new())
                .unwrap();
        }
        motion_voice::export::render_scene("acting", 6.0, &[clip]).unwrap()
    };
    let (plain, derived) = (build(false), build(true));
    assert_eq!(
        serde_json::to_string(&plain.manifest).unwrap(),
        serde_json::to_string(&derived.manifest).unwrap()
    );
    assert_eq!(
        plain.manifest.dialogue[0].alignment_source,
        Some(AlignmentSource::Manual)
    );
    for frame in [24, 29, 30, 36] {
        let (viseme, open) = mouth_pixel_open(&script, &derived, frame);
        let want = frame < 30;
        assert_eq!(open, want, "frame {frame}");
        assert_eq!(viseme == Viseme::A, want, "frame {frame}");
    }
}
