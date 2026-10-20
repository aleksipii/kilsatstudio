//! Golden-property tests for `examples/night-room-reference/`.
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

use motion_export::{render_sequence_frame, SequencePreview};
use motion_scene::SequenceTimeline;
use motion_tts::CancellationToken;

const RATE: f64 = 24_000.0;

struct Fixture {
    expansion: motion_script::ShotExpansion,
    rendered: Vec<motion_voice::export::RenderedScene>,
    art: motion_export::ArtContext,
    timeline: SequenceTimeline,
}

fn fixture() -> Fixture {
    let expansion = support::expansion();
    let document = motion_project::load(&support::example_dir().join("night-room.kilsat"))
        .expect("committed night-room.kilsat; run the example with `write`");
    let rendered = support::rendered_scenes(&expansion.script, &support::example_dir());
    let timeline = SequenceTimeline::compile(&expansion.script).unwrap();
    Fixture {
        art: support::art_context(&document),
        expansion,
        rendered,
        timeline,
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
    // The committed WAV fixtures are exactly the generated placeholder tones.
    let scratch = std::env::temp_dir().join(format!("kilsat-night-room-{}", std::process::id()));
    std::fs::create_dir_all(&scratch).unwrap();
    for (_, _, actor, audio, duration) in support::lines(&expansion) {
        let generated = scratch.join(std::path::Path::new(&audio).file_name().unwrap());
        motion_audio::write_wav(&generated, &support::placeholder_audio(&actor, duration)).unwrap();
        assert_eq!(
            std::fs::read(dir.join(&audio)).unwrap(),
            std::fs::read(&generated).unwrap(),
            "{audio} is stale"
        );
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
    let expected_cuts = [2.4, 5.75, 8.9];
    for (span, cut) in spans.iter().skip(1).zip(expected_cuts) {
        assert_eq!(span.start_sample, (cut * RATE).round() as u64);
    }
    assert_eq!(f.timeline.total_samples(), (11.25 * RATE) as u64);
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
    let dump = std::env::var_os("KILSAT_NIGHT_ROOM_DUMP").map(std::path::PathBuf::from);
    let mut seen_first_frames = vec![];
    for (index, span) in f.timeline.spans().iter().enumerate() {
        let first = frame(&f, span.start_sample);
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
    // 1 doorway: hallway light, father's work shirt, floor.
    assert_color(at(1, (240.0, 100.0)), [72, 176, 124], "hallway opening");
    assert_color(at(1, (320.0, 202.0)), [186, 148, 92], "work shirt");
    assert_color(at(1, (20.0, 345.0)), [30, 22, 46], "floor");
    // 2 boy medium: lamp shade, pyjama top.
    assert_color(at(2, (510.0, 220.0)), [240, 190, 110], "lamp shade");
    assert_color(at(2, (300.0, 245.0)), [60, 100, 190], "pyjama top");
    // 3 father close-up: hall wall and skin.
    assert_color(at(3, (200.0, 100.0)), [28, 70, 64], "hall wall");
    assert_color(at(3, (320.0, 149.0)), [226, 176, 140], "father skin");
    // The father must be absent from the bed shots and the boy from the others.
    assert!(!seen_first_frames[0]
        .as_chunks::<4>()
        .0
        .iter()
        .any(|p| [p[0], p[1], p[2]] == [186, 148, 92]));
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
