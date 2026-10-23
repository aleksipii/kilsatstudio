//! Golden-property tests for `examples/night-room-reference/`, with every integrated
//! feature on: speaker-coloured captions, acting rigs driven by text-estimated
//! visemes, gaze/blinks/brows, lights and the animated phone screen.
//!
//! Frames go through the shared CPU sequence renderer (SVG, then resvg raster).
//! No binary goldens or pixel hashes are stored: antialiasing and font-free
//! vector rasterization may legitimately differ between resvg versions and
//! platforms. Instead the tests assert dimensions, cut times, flat-colour pixels
//! at positions derived from the evaluated camera, determinism across repeated,
//! reversed and shuffled seeks, and bounded differences across cuts.
//!
//! Refresh the committed example files after an intentional change with
//! `cargo run -p motion-export --example night_room_reference -- write`.
//! Set `KILSAT_NIGHT_ROOM_DUMP=<dir>` to write one PNG per shot for review.
mod support;

use motion_export::{render_sequence_frame, CaptionMode, SequencePreview};
use motion_scene::SequenceTimeline;
use motion_tts::CancellationToken;
use std::sync::Arc;

const RATE: f64 = 24_000.0;

struct Fixture {
    expansion: motion_script::ShotExpansion,
    rendered: Vec<motion_voice::export::RenderedScene>,
    art: motion_export::ArtContext,
    timeline: SequenceTimeline,
    preview: SequencePreview,
}

fn fixture() -> Fixture {
    fixture_with_art(|art| art)
}

/// The same project with the lights removed: flat colours for exact-colour probes.
fn unlit_fixture() -> Fixture {
    fixture_with_art(|mut art| {
        art.document.lights.clear();
        art
    })
}

fn fixture_with_art(
    adjust: impl FnOnce(motion_export::ArtContext) -> motion_export::ArtContext,
) -> Fixture {
    let expansion = support::expansion();
    let document = motion_project::load(&support::example_dir().join("night-room.kilsat"))
        .expect("committed night-room.kilsat; run the example with `write`");
    let rendered = support::rendered_scenes(&expansion.script, &support::example_dir());
    let timeline = SequenceTimeline::compile(&expansion.script).unwrap();
    let art = adjust(support::art_context(&document));
    let preview = SequencePreview::new(
        Arc::new(expansion.script.clone()),
        Arc::new(rendered.clone()),
        Some(Arc::new(art.clone())),
        &CancellationToken::new(),
    )
    .unwrap();
    Fixture {
        art,
        expansion,
        rendered,
        timeline,
        preview,
    }
}

fn frame(f: &Fixture, sample: u64) -> Vec<u8> {
    let out = render_sequence_frame(
        &f.expansion.script,
        &f.rendered,
        Some(&f.art),
        sample,
        640,
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!((out.width, out.height), (640, 360));
    assert_eq!(out.rgba.len(), 640 * 360 * 4);
    out.rgba
}

/// A frame through the retained preview with an explicit caption choice.
fn frame_mode(f: &Fixture, sample: u64, mode: CaptionMode) -> Vec<u8> {
    f.preview
        .render_frame_captions(sample, 640, &CancellationToken::new(), mode)
        .unwrap()
        .rgba
}

fn start(f: &Fixture, scene: usize) -> u64 {
    f.timeline.spans()[scene].start_sample
}

fn differing_fraction(a: &[u8], b: &[u8]) -> f64 {
    let n = a.len() / 4;
    let diff = a
        .as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0.iter())
        .filter(|(p, q)| p != q)
        .count();
    diff as f64 / n as f64
}

fn pixel(frame: &[u8], x: f64, y: f64) -> [u8; 3] {
    let (x, y) = (x.round() as usize, y.round() as usize);
    assert!(x < 640 && y < 360, "probe ({x},{y}) outside the frame");
    let i = (y * 640 + x) * 4;
    [frame[i], frame[i + 1], frame[i + 2]]
}

/// Project a world point through the evaluated camera at scene-local `time`.
fn probe(f: &Fixture, scene: usize, time: f64, world: (f64, f64)) -> (f64, f64) {
    let evaluated = motion_scene::evaluate(&f.expansion.script, scene, time).unwrap();
    evaluated.camera.project(world.0, world.1, 640, 360)
}

fn assert_color(actual: [u8; 3], expected: [u8; 3], what: &str) {
    let close = actual
        .iter()
        .zip(expected)
        .all(|(a, e)| (i16::from(*a) - i16::from(e)).abs() <= 2);
    assert!(close, "{what}: expected {expected:?}, got {actual:?}");
}

#[test]
fn committed_example_matches_the_generator() {
    let dir = support::example_dir();
    let expansion = support::expansion();
    let committed = motion_project::load(&dir.join("night-room.kilsat")).unwrap();
    let expected = support::project_document(&expansion);
    assert_eq!(
        committed.source, expected.source,
        "night-room.kilsat is stale: run the example with `write`"
    );
    assert_eq!(committed.art, expected.art);
    assert_eq!(committed.assets, expected.assets);
    // The committed phone-app frames are exactly the generated ones.
    for (id, image) in support::phone_images() {
        assert_eq!(
            std::fs::read(dir.join(format!("assets/{id}.png"))).unwrap(),
            image.png(),
            "assets/{id}.png is stale: run the example with `write`"
        );
    }
    // Locally generated WAV fixtures (gitignored) must equal the generated placeholder tones.
    let scratch = std::env::temp_dir().join(format!("kilsat-night-room-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    for (_, _, actor, audio, duration) in support::lines(&expansion) {
        let generated = scratch.join(std::path::Path::new(&audio).file_name().unwrap());
        motion_audio::write_wav(&generated, &support::placeholder_audio(&actor, duration)).unwrap();
        // `*.wav` is gitignored, so fixtures exist only after a local `write`; when they
        // do, they must match the generator exactly.
        if dir.join(&audio).is_file() {
            assert_eq!(
                std::fs::read(dir.join(&audio)).unwrap(),
                std::fs::read(&generated).unwrap(),
                "{audio} is stale"
            );
        }
    }
    let _ = std::fs::remove_dir_all(scratch);
}

#[test]
fn shots_cut_on_dialogue_starts_on_the_sample_clock() {
    let f = fixture();
    let script = &f.expansion.script;
    assert_eq!((script.project.width, script.project.height), (640, 360));
    let ids: Vec<_> = script.scenes.iter().map(|s| s.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "bed_closeup",
            "doorway_wide",
            "boy_phone_medium",
            "dad_closeup"
        ]
    );
    // Hard cuts: every shot starts exactly where the previous one ends.
    let spans = f.timeline.spans();
    for pair in spans.windows(2) {
        assert_eq!(pair[0].outgoing_overlap, 0);
        assert_eq!(
            pair[0].start_sample + pair[0].duration_samples,
            pair[1].start_sample
        );
    }
    let expected_cuts = [2.4, 6.45, 9.4];
    for (span, cut) in spans.iter().skip(1).zip(expected_cuts) {
        assert_eq!(span.start_sample, (cut * RATE).round() as u64);
    }
    assert_eq!(f.timeline.total_samples(), (11.65 * RATE) as u64);
    // The plan agrees with the sample-clock timeline to the sample.
    for (planned, span) in f.expansion.plan.iter().zip(spans) {
        assert_eq!((planned.start * RATE).round() as u64, span.start_sample);
    }
    // Each line starts lead_in (0.15 s) after its cut, in both the plan and the
    // mixed audio: silence before, signal after, on the same clock.
    for (index, planned) in f.expansion.plan.iter().enumerate().skip(1) {
        let line_start = planned.dialogue_start.unwrap();
        assert!((line_start - planned.start - 0.15).abs() < 1e-9);
        let manifest = &f.rendered[index].manifest;
        assert_eq!(manifest.dialogue.len(), 1);
        assert_eq!(manifest.dialogue[0].start_frame, 3600);
        let audio = &f.rendered[index].audio.samples;
        assert!(audio[..3600].iter().all(|s| *s == 0.0));
        assert!(audio[3600..3700].iter().any(|s| *s != 0.0));
    }
    // The first shot is a silent reaction hold.
    assert!(f.rendered[0].manifest.dialogue.is_empty());
}

#[test]
fn every_shot_renders_non_blank_frames_with_expected_flat_colours() {
    let f = fixture();
    let flat = unlit_fixture();
    let dump = std::env::var_os("KILSAT_NIGHT_ROOM_DUMP").map(std::path::PathBuf::from);
    let mut seen_first_frames = vec![];
    for (index, span) in f.timeline.spans().iter().enumerate() {
        let first = frame_mode(&flat, span.start_sample, CaptionMode::Hide);
        let middle = frame(&f, span.start_sample + span.duration_samples / 2);
        for (name, image) in [("first", &first), ("middle", &middle)] {
            let distinct: std::collections::BTreeSet<_> = image
                .as_chunks::<4>()
                .0
                .iter()
                .map(|p| [p[0], p[1], p[2]])
                .collect();
            assert!(
                distinct.len() >= 6,
                "shot {index} {name} frame looks blank: {} colours",
                distinct.len()
            );
            assert!(image.as_chunks::<4>().0.iter().all(|p| p[3] == 255));
        }
        if let Some(dir) = &dump {
            std::fs::create_dir_all(dir).unwrap();
            let pixmap = resvg::tiny_skia::Pixmap::from_vec(
                middle.clone(),
                resvg::tiny_skia::IntSize::from_wh(640, 360).unwrap(),
            )
            .unwrap();
            pixmap
                .save_png(dir.join(format!("{index}_{}.png", f.expansion.plan[index].scene_id)))
                .unwrap();
        }
        seen_first_frames.push(first);
    }
    // Key colours at world points located through the evaluated camera at t = 0
    // (rest pose: the head joint track and push have not moved anything yet).
    let at = |scene: usize, world: (f64, f64)| {
        let (x, y) = probe(&f, scene, 0.0, world);
        pixel(&seen_first_frames[scene], x, y)
    };
    // 0 bed close-up: headboard, beanie crown (head joint at 300 - 60).
    assert_color(at(0, (200.0, 200.0)), [64, 44, 52], "headboard");
    assert_color(at(0, (320.0, 185.0)), [210, 60, 70], "beanie crown");
    // This exposed headboard area used to be covered by the ambiguous blue oval.
    assert_color(at(0, (275.0, 210.0)), [64, 44, 52], "clear headboard");
    assert_color(at(2, (275.0, 210.0)), [64, 44, 52], "clear headboard");
    // 1 doorway: hallway light, father's work shirt, floor.
    assert_color(at(1, (240.0, 100.0)), [72, 176, 124], "hallway opening");
    assert_color(at(1, (320.0, 202.0)), [76, 66, 65], "work shirt");
    assert_color(at(1, (20.0, 345.0)), [30, 22, 46], "floor");
    // 2 boy medium: lamp shade, pyjama top.
    assert_color(at(2, (510.0, 220.0)), [240, 190, 110], "lamp shade");
    assert_color(at(2, (300.0, 245.0)), [60, 100, 190], "pyjama top");
    // 3 father close-up: hall wall and skin.
    assert_color(at(3, (200.0, 100.0)), [28, 70, 64], "hall wall");
    assert_color(at(3, (320.0, 149.0)), [154, 119, 109], "father skin");
    // Bold black outlines (`behind` mode, 1.8 units outside the round heads): boy
    // head ellipse centre (320, 219) rx 28; father centre (320, 144) rx 31.
    let outline = |p: [u8; 3], what: &str| {
        assert!(p.iter().all(|c| *c < 24), "{what} outline: {p:?}");
    };
    outline(at(0, (348.9, 219.0)), "boy head");
    outline(at(0, (291.1, 219.0)), "boy head");
    outline(at(3, (351.9, 144.0)), "father head");
    outline(at(3, (288.1, 144.0)), "father head");
    // The father must be absent from the bed shots and the boy from the others.
    assert!(!seen_first_frames[0]
        .as_chunks::<4>()
        .0
        .iter()
        .any(|p| [p[0], p[1], p[2]] == [76, 66, 65]));
}

#[test]
fn cuts_change_the_picture_while_frames_inside_a_shot_stay_continuous() {
    let f = fixture();
    for scene in 1..f.timeline.spans().len() {
        let cut = start(&f, scene);
        let before = frame(&f, cut - 1);
        let after = frame(&f, cut);
        assert!(
            differing_fraction(&before, &after) > 0.5,
            "no visible cut into shot {scene}"
        );
        // Two samples inside the outgoing shot are visually identical.
        let earlier = frame(&f, cut - 2);
        assert!(
            differing_fraction(&earlier, &before) < 0.01,
            "outgoing shot {scene} is not continuous"
        );
    }
    // The slow push and head nod move the picture over the shot.
    let first = frame(&f, 0);
    let late = frame(&f, 2 * 24_000);
    assert!(differing_fraction(&first, &late) > 0.01);
}

#[test]
fn seeks_are_deterministic_when_repeated_reversed_and_shuffled() {
    let f = fixture();
    let total = f.timeline.total_samples();
    let samples: Vec<u64> = (0..24).map(|i| i * 11_251 % total).collect();
    let forward: Vec<_> = samples.iter().map(|s| frame(&f, *s)).collect();
    // Reversed order, fresh renders.
    for (sample, expected) in samples.iter().zip(&forward).rev() {
        assert_eq!(&frame(&f, *sample), expected, "reverse seek {sample}");
    }
    // A fixed pseudo-random order, through the retained-preview path too.
    let preview = SequencePreview::new(
        std::sync::Arc::new(f.expansion.script.clone()),
        std::sync::Arc::new(f.rendered.clone()),
        Some(std::sync::Arc::new(f.art.clone())),
        &CancellationToken::new(),
    )
    .unwrap();
    let mut order: Vec<usize> = (0..samples.len()).collect();
    let mut state = 0x9E37_79B9_u32;
    for i in (1..order.len()).rev() {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        order.swap(i, state as usize % (i + 1));
    }
    for i in order {
        let out = preview
            .render_frame(samples[i], 640, &CancellationToken::new())
            .unwrap();
        assert_eq!(out.rgba, forward[i], "shuffled seek {}", samples[i]);
    }
    // Scene evaluation alone is also stateless.
    let a = motion_scene::evaluate(&f.expansion.script, 2, 1.0).unwrap();
    let _ = motion_scene::evaluate(&f.expansion.script, 0, 2.0).unwrap();
    let b = motion_scene::evaluate(&f.expansion.script, 2, 1.0).unwrap();
    assert_eq!(a, b);
}

#[test]
fn camera_framing_places_the_subject_near_the_frame_centre() {
    let f = fixture();
    // Close-ups and the medium shot centre their target (plus offset) on screen.
    for (scene, world) in [
        (0, (320.0, 220.0)),
        (2, (320.0, 240.0)),
        (3, (320.0, 150.0)),
    ] {
        for time in [0.0, 1.0] {
            let (x, y) = probe(&f, scene, time, world);
            assert!(
                (x - 320.0).abs() < 1e-6 && (y - 180.0).abs() < 1e-6,
                "{scene} {time}"
            );
        }
    }
    // The wide establishing shot is un-zoomed.
    let wide = motion_scene::evaluate(&f.expansion.script, 1, 0.5).unwrap();
    assert_eq!(
        (wide.camera.x, wide.camera.y, wide.camera.zoom),
        (0.0, 0.0, 1.0)
    );
}

// ---------------------------------------------------------------------------
// Integration: captions, acting, lighting, animated prop and estimated visemes.
// ---------------------------------------------------------------------------

const BOY: usize = 2;
const DOORWAY: usize = 1;
const DAD_CLOSE: usize = 3;

/// Global sample of scene-local `time` seconds in `scene`.
fn at_time(f: &Fixture, scene: usize, time: f64) -> u64 {
    start(f, scene) + (time * RATE).round() as u64
}

type Rect = (usize, usize, usize, usize);

/// Screen rectangle `(x0, y0, x1, y1)` (pixels) of a world box at scene-local `time`.
fn screen_rect(f: &Fixture, scene: usize, time: f64, centre: (f64, f64), half: (f64, f64)) -> Rect {
    let (x0, y0) = probe(f, scene, time, (centre.0 - half.0, centre.1 - half.1));
    let (x1, y1) = probe(f, scene, time, (centre.0 + half.0, centre.1 + half.1));
    (
        x0.floor() as usize,
        y0.floor() as usize,
        x1.ceil() as usize,
        y1.ceil() as usize,
    )
}

fn pixels_in(frame: &[u8], rect: Rect) -> impl Iterator<Item = (usize, usize, [u8; 3])> + '_ {
    (rect.1..rect.3.min(360)).flat_map(move |y| {
        (rect.0..rect.2.min(640)).map(move |x| {
            let i = (y * 640 + x) * 4;
            (x, y, [frame[i], frame[i + 1], frame[i + 2]])
        })
    })
}

fn near(a: [u8; 3], b: [u8; 3], tolerance: i16) -> bool {
    a.iter()
        .zip(b)
        .all(|(a, b)| (i16::from(*a) - i16::from(b)).abs() <= tolerance)
}

/// Pupil-dark pixels (count and centroid) inside a screen rectangle.
fn dark(frame: &[u8], rect: Rect) -> (usize, f64, f64) {
    let (mut n, mut sx, mut sy) = (0usize, 0.0, 0.0);
    for (x, y, p) in pixels_in(frame, rect) {
        if p.iter().all(|c| *c < 70) {
            n += 1;
            sx += x as f64;
            sy += y as f64;
        }
    }
    if n == 0 {
        (0, 0.0, 0.0)
    } else {
        (n, sx / n as f64, sy / n as f64)
    }
}

/// Non-skin, non-moustache pixels in the mouth box (the drawn mouth area).
fn mouth_area(frame: &[u8], rect: Rect) -> usize {
    pixels_in(frame, rect)
        .filter(|(_, _, p)| !near(*p, [154, 119, 109], 14) && !near(*p, [50, 36, 30], 14))
        .count()
}

fn speaking_scenes(f: &Fixture) -> Vec<usize> {
    (0..f.rendered.len())
        .filter(|i| !f.rendered[*i].manifest.dialogue.is_empty())
        .collect()
}

#[test]
fn every_integrated_feature_is_configured_in_the_one_project() {
    let f = fixture();
    let script = &f.expansion.script;
    // Captions: white father, yellow boy, uppercase.
    let style = script.project.captions.as_ref().expect("captions style");
    assert!(style.enabled && style.uppercase);
    assert_eq!(style.speaker_fill["DAD"], "#ffffff");
    assert_eq!(style.speaker_fill["STAN"], "#ffd400");
    // Acting rigs: every viseme but `rest` maps to a mouth variant; eyes, blink, brows.
    for actor in ["STAN", "DAD"] {
        let rig = script.characters[actor].rig.as_ref().unwrap();
        let visemes = rig.visemes.as_ref().expect("viseme map");
        assert_eq!(visemes.part, "mouth");
        assert_eq!(
            visemes.map.len(),
            12,
            "{actor}: all non-rest visemes mapped"
        );
        assert!(rig.eyes.as_ref().unwrap().blink.is_some());
        for emotion in ["sad", "surprised", "angry"] {
            assert!(rig.expressions.contains_key(emotion), "{actor} {emotion}");
        }
        rig.prepare().unwrap();
    }
    // Lights: tint + vignette in every shot; screen glow tied to the phone prop.
    let art = &f.art.document;
    art.validate().unwrap();
    for scene in &script.scenes {
        let lights: Vec<_> = art
            .lights
            .iter()
            .filter(|l| l.scene_id == scene.id)
            .collect();
        assert!(lights
            .iter()
            .any(|l| matches!(l.source, motion_art::LightSource::Ambient { .. })));
        assert!(lights
            .iter()
            .any(|l| matches!(l.source, motion_art::LightSource::Vignette { .. })));
        for light in &lights {
            light.validate_scene_duration(scene.duration).unwrap();
        }
    }
    let glow_scenes: Vec<_> = art
        .lights
        .iter()
        .filter_map(|l| match &l.source {
            motion_art::LightSource::Radial {
                screen_glow: Some(glow),
                ..
            } => Some((l.scene_id.clone(), glow.layer_id.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(
        glow_scenes,
        [
            (
                "bed_closeup".to_string(),
                "bed_closeup_phone_screen".to_string()
            ),
            (
                "boy_phone_medium".to_string(),
                "boy_phone_medium_phone_screen".to_string()
            )
        ]
    );
    for (scene, layer) in &glow_scenes {
        let prop = art.layers.iter().find(|l| &l.id == layer).unwrap();
        assert_eq!(&prop.scene_id, scene);
        assert!(matches!(
            &prop.content,
            motion_art::Content::AnimatedImage { frames, .. } if frames.len() == 3
        ));
    }
    assert!(art.lights.iter().any(|l| l.scene_id == "boy_phone_medium"
        && l.flicker.is_some()
        && matches!(
            l.source,
            motion_art::LightSource::Radial { color, .. } if color.r > color.b
        )));
    // Every dialogue clip carries a text-estimated mouth track (never alignment).
    for scene in speaking_scenes(&f) {
        for clip in &f.rendered[scene].manifest.dialogue {
            assert!(matches!(
                &clip.alignment_source,
                Some(motion_lipsync::AlignmentSource::Estimated { tool, .. })
                    if tool == "kilsat-english-estimator"
            ));
            let distinct: std::collections::BTreeSet<_> = clip
                .visemes
                .iter()
                .map(|v| format!("{:?}", v.viseme))
                .collect();
            assert!(distinct.len() >= 5, "{} visemes only", distinct.len());
            let end = clip.start_frame + clip.duration_frames;
            assert!(clip
                .visemes
                .iter()
                .all(|v| v.start_frame >= clip.start_frame && v.end_frame <= end));
        }
    }
}

#[test]
fn captions_differ_from_hidden_captions_only_in_the_caption_area() {
    let f = fixture();
    for scene in speaking_scenes(&f) {
        let clip = &f.rendered[scene].manifest.dialogue[0];
        let sample = start(&f, scene) + clip.start_frame + clip.duration_frames / 2;
        let shown = frame_mode(&f, sample, CaptionMode::Follow);
        let hidden = frame_mode(&f, sample, CaptionMode::Hide);
        let changed: Vec<(usize, usize)> = (0..640 * 360)
            .filter(|i| shown[i * 4..i * 4 + 4] != hidden[i * 4..i * 4 + 4])
            .map(|i| (i % 640, i / 640))
            .collect();
        assert!(changed.len() > 500, "scene {scene}: caption not drawn");
        let (min_x, max_x) = changed
            .iter()
            .fold((640, 0), |(a, b), (x, _)| (a.min(*x), b.max(*x)));
        let min_y = changed.iter().map(|(_, y)| *y).min().unwrap();
        assert!(
            min_y >= 250,
            "scene {scene}: caption reaches up to y={min_y}"
        );
        // The opaque backing spans the viewport below the glyphs, separating
        // dialogue from the blanket even while the camera pushes in.
        assert_eq!((min_x, max_x), (0, 639));
        assert_color(pixel(&shown, 10.0, 355.0), [21, 18, 31], "caption band");
        assert!((min_x + max_x) / 2 > 280 && (min_x + max_x) / 2 < 360);
        // Speaker colours: yellow for the boy, white for the father.
        let region = (0, 250, 640, 360);
        let yellow = pixels_in(&shown, region)
            .filter(|p| near(p.2, [255, 212, 0], 6))
            .count();
        let white = pixels_in(&shown, region)
            .filter(|p| near(p.2, [255, 255, 255], 3))
            .count();
        if scene == BOY {
            assert!(yellow > 150, "yellow {yellow}");
            assert!(white < 10, "white {white}");
        } else {
            assert!(white > 150, "white {white}");
            assert_eq!(yellow, 0);
        }
    }
}

#[test]
fn caption_window_matches_the_dialogue_clip_on_the_sample_clock() {
    let f = fixture();
    for scene in speaking_scenes(&f) {
        let clip = &f.rendered[scene].manifest.dialogue[0];
        let first = start(&f, scene) + clip.start_frame;
        let end = first + clip.duration_frames;
        let differs = |sample: u64| {
            frame_mode(&f, sample, CaptionMode::Follow) != frame_mode(&f, sample, CaptionMode::Hide)
        };
        assert!(
            !differs(first - 1),
            "scene {scene}: caption before the line"
        );
        assert!(
            differs(first),
            "scene {scene}: caption missing at the first sample"
        );
        assert!(
            differs(end - 1),
            "scene {scene}: caption gone before the last sample"
        );
        assert!(!differs(end), "scene {scene}: caption after the line");
        assert!(end < start(&f, scene) + f.timeline.spans()[scene].duration_samples);
    }
}

#[test]
fn every_dialogue_line_lays_out_without_missing_glyphs() {
    let f = fixture();
    let style = f.expansion.script.project.captions.as_ref().unwrap();
    for planned in &f.expansion.plan {
        for event in &f.expansion.script.scenes[planned.scene_index].events {
            if let motion_script::Event::Speak { text, .. } = event {
                motion_render::validate_caption(style, text, 640, 360).unwrap();
            }
        }
    }
    // `Show` fails loudly on undrawable text, so a clean render also covers wrapping.
    for scene in speaking_scenes(&f) {
        let sample = at_time(&f, scene, 1.0);
        f.preview
            .render_frame_captions(sample, 640, &CancellationToken::new(), CaptionMode::Show)
            .unwrap();
    }
}

#[test]
fn mouth_variants_follow_the_text_estimated_visemes() {
    use motion_lipsync::Viseme;
    let f = unlit_fixture();
    let clip = &f.rendered[DAD_CLOSE].manifest.dialogue[0];
    let hide = |sample| frame_mode(&f, sample, CaptionMode::Hide);
    // Mouth box: Dad is placed at (320, 260), head joint (320, 174), mouth centre y=169.
    let area = |sample: u64| {
        let time = (sample - start(&f, DAD_CLOSE)) as f64 / RATE;
        let rect = screen_rect(&f, DAD_CLOSE, time, (320.0, 169.0), (9.0, 6.0));
        mouth_area(&hide(sample), rect)
    };
    let rest = area(start(&f, DAD_CLOSE) + 1_000);
    assert!(rest > 10, "resting mouth should be drawn, got {rest}");
    let mut areas = std::collections::BTreeMap::new();
    for interval in &clip.visemes {
        if interval.end_frame - interval.start_frame < 960 {
            continue;
        }
        let mid = start(&f, DAD_CLOSE) + (interval.start_frame + interval.end_frame) / 2;
        areas
            .entry(format!("{:?}", interval.viseme))
            .or_insert_with(Vec::new)
            .push(area(mid));
    }
    let distinct: std::collections::BTreeSet<usize> = areas.values().flatten().copied().collect();
    assert!(
        distinct.len() >= 4,
        "only {distinct:?} mouth areas over {areas:?}"
    );
    let open = format!("{:?}", Viseme::A);
    let wide_open = areas
        .get(&open)
        .expect("the line has an open-vowel interval");
    assert!(
        wide_open.iter().all(|a| *a as f64 > rest as f64 * 1.5),
        "open mouth {wide_open:?} vs rest {rest}"
    );
    if let Some(closed) = areas.get(&format!("{:?}", Viseme::Closed)) {
        assert!(closed.iter().all(|a| a < wide_open.iter().min().unwrap()));
    }
    // Same samples, repeated and out of order: identical frames.
    let probes: Vec<u64> = clip
        .visemes
        .iter()
        .map(|v| start(&f, DAD_CLOSE) + v.start_frame)
        .collect();
    let forward: Vec<_> = probes.iter().map(|s| hide(*s)).collect();
    for (s, expected) in probes.iter().zip(&forward).rev() {
        assert_eq!(&hide(*s), expected);
    }
}

fn blink_at(f: &Fixture, actor: &str, time: f64) -> bool {
    f.expansion.script.characters[actor]
        .rig
        .as_ref()
        .unwrap()
        .prepare()
        .unwrap()
        .blink_active(actor, time)
}

#[test]
fn eyes_blink_in_every_shot_and_pupils_follow_the_gaze() {
    let f = unlit_fixture();
    let hide = |sample| frame_mode(&f, sample, CaptionMode::Hide);
    // (scene, actor, left-eye world centre with the actor at its shot position)
    let cases = [
        (BOY, "STAN", (310.0, 217.5)),
        (DAD_CLOSE, "DAD", (308.0, 143.0)),
    ];
    for (scene, actor, eye) in cases {
        let duration = f.expansion.script.scenes[scene].duration;
        let frames = (duration * 24.0) as usize;
        let times = || (0..frames).map(|i| i as f64 / 24.0);
        let blink = times().find(|t| blink_at(&f, actor, *t));
        let open = times().find(|t| !blink_at(&f, actor, *t));
        let (blink, open) = (blink.expect("a blink in the shot"), open.unwrap());
        let count = |t: f64| {
            let rect = screen_rect(&f, scene, t, eye, (5.0, 4.2));
            dark(&hide(at_time(&f, scene, t)), rect).0
        };
        assert!(count(open) > 4, "{actor}: pupil missing when open");
        assert_eq!(count(blink), 0, "{actor}: pupil visible during blink");
        // Blinks are rare: well under a third of the frames.
        let blinking = times().filter(|t| blink_at(&f, actor, *t)).count();
        assert!(
            blinking * 3 < frames,
            "{actor}: {blinking}/{frames} frames blink"
        );
    }
    // Boy: at 0.5 s he looks at the phone below his chin; at 1.3 s up and right.
    let centroid = |t: f64| {
        let rect = screen_rect(&f, BOY, t, (310.0, 217.5), (7.0, 4.4));
        let (n, x, y) = dark(&hide(at_time(&f, BOY, t)), rect);
        assert!(n > 4, "pupil missing at {t}");
        (x, y)
    };
    let (look_x, look_y) = centroid(1.3);
    let (phone_x, phone_y) = centroid(0.5);
    assert!(
        look_x > phone_x + 1.0,
        "looking right: {look_x} vs {phone_x}"
    );
    assert!(
        phone_y > look_y + 1.0,
        "phone is below: {phone_y} vs {look_y}"
    );
    // Father in his close-up looks left and down (towards the bed) of the eye centre.
    let rect = screen_rect(&f, DAD_CLOSE, 0.5, (308.0, 143.0), (7.0, 6.0));
    let (n, x, y) = dark(&hide(at_time(&f, DAD_CLOSE, 0.5)), rect);
    assert!(n > 4);
    let (cx, cy) = probe(&f, DAD_CLOSE, 0.5, (308.0, 143.0));
    assert!(
        x < cx - 1.0 && y > cy + 0.3,
        "pupil at ({x:.1},{y:.1}), centre ({cx:.1},{cy:.1})"
    );
}

#[test]
fn raised_brows_appear_for_the_surprised_father_in_the_doorway() {
    let f = unlit_fixture();
    let brow = |centre_y: f64| {
        let frame = frame_mode(&f, at_time(&f, DOORWAY, 0.5), CaptionMode::Hide);
        let rect = screen_rect(&f, DOORWAY, 0.5, (320.0, centre_y), (12.0, 1.2));
        // brow colour (50, 36, 30)
        pixels_in(&frame, rect)
            .filter(|p| near(p.2, [50, 36, 30], 6))
            .count()
    };
    // Father placed at (320, 222): head joint y=136, brows at local y -40.5, raised -44.5.
    let raised = brow(136.0 - 44.5);
    let neutral_position = brow(136.0 - 40.5);
    assert!(raised > 0, "raised brow missing");
    assert!(
        neutral_position < raised,
        "brow should sit in the raised position ({neutral_position} vs {raised})"
    );
}

#[test]
fn phone_screen_animates_and_its_glow_follows_the_shown_frame() {
    let f = fixture();
    let unlit = unlit_fixture();
    // Frames last 0.6 s and loop: t=0.3 -> frame 0, 0.9 -> frame 1, 1.5 -> frame 2, 2.1 -> frame 0.
    let screen_pixel = |f: &Fixture, t: f64| {
        let frame = frame_mode(f, at_time(f, BOY, t), CaptionMode::Hide);
        let screen = support::PHONE_SCREEN;
        let world = (
            screen.x + screen.width / 2.0,
            screen.y + screen.height * 0.04,
        );
        let (x, y) = probe(f, BOY, t, world);
        pixel(&frame, x, y)
    };
    let (a, b, c) = (
        screen_pixel(&unlit, 0.3),
        screen_pixel(&unlit, 0.9),
        screen_pixel(&unlit, 1.5),
    );
    assert!(
        a != b && b != c && a != c,
        "frames identical: {a:?} {b:?} {c:?}"
    );
    // Looped: the same frame again after 1.8 s (the push moves the probe slightly).
    assert!(
        near(screen_pixel(&unlit, 2.1), a, 24),
        "{:?} vs {a:?}",
        screen_pixel(&unlit, 2.1)
    );
    // Backlight: measure exposed headboard beside the phone, not an opaque actor.
    // Subtract the same frame with only the phone light removed to isolate its
    // contribution from camera movement, room tint and the lamp.
    let no_phone = fixture_with_art(|mut art| {
        art.document
            .lights
            .retain(|l| !l.id.ends_with("phone_glow"));
        art
    });
    let glow = |t: f64| {
        let frame = frame_mode(&f, at_time(&f, BOY, t), CaptionMode::Hide);
        let without = frame_mode(&no_phone, at_time(&f, BOY, t), CaptionMode::Hide);
        let rect = screen_rect(&f, BOY, t, (355.0, 250.0), (4.0, 4.0));
        pixels_in(&frame, rect)
            .zip(pixels_in(&without, rect))
            .map(|((_, _, a), (_, _, b))| i64::from(a[2]) - i64::from(b[2]))
            .sum::<i64>()
    };
    let (dark, bright) = (glow(0.3), glow(1.5));
    assert!(bright > dark && bright > 0, "{bright} vs {dark}");
}

#[test]
fn lights_tint_darken_and_glow_the_expected_areas() {
    let lit = fixture();
    let flat = unlit_fixture();
    let both = |scene: usize, t: f64, world: (f64, f64)| {
        let sample = at_time(&lit, scene, t);
        let (x, y) = probe(&lit, scene, t, world);
        let a = pixel(&frame_mode(&lit, sample, CaptionMode::Hide), x, y);
        let b = pixel(&frame_mode(&flat, sample, CaptionMode::Hide), x, y);
        (a, b)
    };
    let lum = |p: [u8; 3]| u32::from(p[0]) * 3 + u32::from(p[1]) * 6 + u32::from(p[2]);
    // Ambient multiply + vignette: every shot's far corner is much darker than flat art.
    for scene in 0..4 {
        let sample = at_time(&lit, scene, 0.5);
        let a = pixel(&frame_mode(&lit, sample, CaptionMode::Hide), 2.0, 2.0);
        let b = pixel(&frame_mode(&flat, sample, CaptionMode::Hide), 2.0, 2.0);
        assert!(lum(a) * 2 < lum(b), "scene {scene}: corner {a:?} vs {b:?}");
    }
    // Warm lamp (boy shot): red outweighs blue next to the lamp shade.
    let (a, b) = both(BOY, 1.0, (505.0, 225.0));
    assert!(a[0] > a[2], "lamp area {a:?} vs flat {b:?}");
    // Green hallway light: removing just that light lowers green against blue/red.
    let no_hall = fixture_with_art(|mut art| {
        art.document
            .lights
            .retain(|l| !l.id.ends_with("hallway_light"));
        art
    });
    for (scene, world) in [(DOORWAY, (270.0, 180.0)), (DAD_CLOSE, (270.0, 140.0))] {
        let sample = at_time(&lit, scene, 0.5);
        let (x, y) = probe(&lit, scene, 0.5, world);
        let with = pixel(&frame_mode(&lit, sample, CaptionMode::Hide), x, y);
        let without = pixel(&frame_mode(&no_hall, sample, CaptionMode::Hide), x, y);
        assert!(
            i16::from(with[1]) - i16::from(without[1]) >= 8
                && i16::from(with[1]) - i16::from(with[0])
                    > i16::from(without[1]) - i16::from(without[0]),
            "scene {scene}: hallway {with:?} vs without {without:?}"
        );
    }
    // The lamp flickers: its glow changes over time while the rest of the frame is steady.
    let sum = |t: f64| -> u64 {
        let frame = frame_mode(&lit, at_time(&lit, BOY, t), CaptionMode::Hide);
        let rect = screen_rect(&lit, BOY, t, (505.0, 225.0), (20.0, 20.0));
        pixels_in(&frame, rect).map(|p| u64::from(p.2[0])).sum()
    };
    let samples: std::collections::BTreeSet<u64> =
        (0..12).map(|i| sum(0.1 + f64::from(i) * 0.1)).collect();
    assert!(samples.len() > 3, "lamp light is static: {samples:?}");
}

#[test]
fn preview_and_png_sequence_export_agree_with_everything_on() {
    let f = fixture();
    let dir = std::env::temp_dir().join(format!("kilsat-night-room-export-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let report = motion_export::export_png_sequence(
        &f.expansion.script,
        &f.rendered,
        Some(&f.art),
        &dir,
        &CancellationToken::new(),
        |_| {},
    )
    .unwrap();
    assert_eq!(report.frame_count, 280); // 11.65 s at 24 fps, rounded up
                                         // Lead-in, mid-line and between-line frames of every shot.
    for n in [0u64, 30, 62, 80, 120, 156, 170, 200, 232, 250, 275] {
        let png =
            resvg::tiny_skia::Pixmap::load_png(dir.join(format!("frame-{n:06}.png"))).unwrap();
        let preview = frame_mode(&f, n * 1000, CaptionMode::Follow);
        assert_eq!(
            png.data(),
            preview.as_slice(),
            "frame {n} differs between export and preview"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn shuffled_seeks_across_the_integrated_features_are_deterministic() {
    let f = fixture();
    let samples: Vec<u64> = support::representative_samples(&f.expansion)
        .into_iter()
        .chain((0..8).map(|i| 3_001 + i * 33_333 % f.timeline.total_samples()))
        .collect();
    let forward: Vec<_> = samples
        .iter()
        .map(|s| frame_mode(&f, *s, CaptionMode::Follow))
        .collect();
    for (s, expected) in samples.iter().zip(&forward).rev() {
        assert_eq!(
            &frame_mode(&f, *s, CaptionMode::Follow),
            expected,
            "sample {s}"
        );
    }
    // A fresh fixture (new retained preview, new text estimates) gives the same pixels.
    let other = fixture();
    for (s, expected) in samples.iter().zip(&forward) {
        assert_eq!(
            &frame_mode(&other, *s, CaptionMode::Follow),
            expected,
            "fresh {s}"
        );
    }
}

/// Rebuilds the sequence around the real Kokoro WAVs. Ignored by default: the WAVs
/// are gitignored and produced by `scripts/night_room_real_speech.py` (offline, local
/// CPU). Set `KILSAT_NIGHT_ROOM_REAL_AUDIO` to the output directory (default
/// `~/Library/Application Support/Kilsat Studio/night-room-audio`), then run
/// `cargo test -p motion-export --test night_room_reference -- --ignored`.
/// Structural checks only: it says nothing about voice quality or lip-sync quality.
#[test]
#[ignore = "needs real Kokoro WAVs from scripts/night_room_real_speech.py"]
fn real_speech_rebuilds_the_sequence_and_renders_everything() {
    let dir = std::env::var_os("KILSAT_NIGHT_ROOM_REAL_AUDIO")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var_os("HOME").expect("HOME"))
                .join("Library/Application Support/Kilsat Studio/night-room-audio")
        });
    let durations = support::wav_durations(&dir);
    let (expansion, rendered) = support::build(Some(&dir));
    let timeline = SequenceTimeline::compile(&expansion.script).unwrap();
    let document = support::project_document(&expansion);
    let art = support::art_context(&document);
    let preview = SequencePreview::new(
        Arc::new(expansion.script.clone()),
        Arc::new(rendered.clone()),
        Some(Arc::new(art)),
        &CancellationToken::new(),
    )
    .unwrap();
    for (planned, span) in expansion.plan.iter().zip(timeline.spans()) {
        assert_eq!((planned.start * RATE).round() as u64, span.start_sample);
    }
    for (_, at, _, audio, planned_duration) in support::lines(&expansion) {
        // The shot list was re-timed to the WAV length.
        let wav = durations[&audio];
        assert!((planned_duration - wav).abs() < 1e-9, "{audio}");
        assert!(wav > 0.5 && wav < 8.0, "{audio}: {wav} s");
        assert!((at - 0.15).abs() < 1e-9);
    }
    for (index, sample) in support::representative_samples(&expansion)
        .into_iter()
        .enumerate()
    {
        let render = |mode| {
            preview
                .render_frame_captions(sample, 640, &CancellationToken::new(), mode)
                .unwrap()
                .rgba
        };
        let shown = render(CaptionMode::Follow);
        assert_eq!(shown.len(), 640 * 360 * 4);
        assert_eq!(
            shown != render(CaptionMode::Hide),
            index > 0,
            "shot {index} caption"
        );
        for clip in &rendered[index].manifest.dialogue {
            let (switches, intervals) = support::mouth_switches(clip);
            assert!(
                intervals >= 8 && switches >= 8,
                "{intervals} intervals, {switches} switches"
            );
            assert!(matches!(
                clip.alignment_source,
                Some(motion_lipsync::AlignmentSource::Estimated { .. })
            ));
        }
    }
    // Real speech is not silent.
    for scene in &rendered[1..] {
        assert!(scene.audio.samples.iter().any(|s| s.abs() > 0.05));
    }
}
