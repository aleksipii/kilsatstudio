//! Outlined and rounded rig parts through the shared SVG path: PNG pixels show
//! black edges where expected, rounded corners, and agree with preview frames.
use super::*;
use std::sync::Arc;

const SOURCE: &str = "project: {title: Outlines, fps: 24, width: 256, height: 256}
characters:
  PUPPET:
    asset: custom
    rig:
      version: 1
      id: puppet
      joints: [{id: root, parent: null}, {id: head, parent: root, y: -1}]
      parts:
        - {id: body, joint: root, ellipse: {cy: 30, rx: 30, ry: 20}, color: [40, 160, 60, 255], outline: {width: 3, mode: behind}}
        - {id: head, joint: head, ellipse: {cy: 1, rx: 20, ry: 20}, color: [240, 200, 160, 255], outline: {width: 3, mode: behind}}
        - {id: badge, joint: root, points: [[-60, -60], [-30, -60], [-30, -30], [-60, -30]], smooth: 8, color: [40, 60, 200, 255], outline: {width: 2}}
scenes: [{id: s, duration: 2, events: [{type: move, at: 0, actor: PUPPET, x: 128, y: 128, duration: 0.01}]}]
";

fn script() -> Script {
    motion_script::parse(SOURCE).unwrap()
}
fn raster(script: &Script, time: f64) -> resvg::tiny_skia::Pixmap {
    let scene = motion_scene::evaluate(script, 0, time).unwrap();
    let svg = motion_render::render_svg(&scene, 256, 256).unwrap();
    resvg::tiny_skia::Pixmap::decode_png(&media::rasterize(&svg).unwrap()).unwrap()
}
/// Pixel centred on actor-local `(x, y)`; the actor stands at (128, 128).
fn at(p: &resvg::tiny_skia::Pixmap, x: f64, y: f64) -> [u8; 3] {
    let c = p
        .pixel((128.0 + x).floor() as u32, (128.0 + y).floor() as u32)
        .unwrap();
    [c.red(), c.green(), c.blue()]
}
fn black(p: [u8; 3]) -> bool {
    p.iter().all(|c| *c < 16)
}
const SKIN: [u8; 3] = [240, 200, 160];
const GREEN: [u8; 3] = [40, 160, 60];
const BLUE: [u8; 3] = [40, 60, 200];
/// The default scene background (#181f2c).
const BACKGROUND: [u8; 3] = [24, 31, 44];

#[test]
fn outlines_are_black_at_edges_and_sit_under_later_fills() {
    let script = script();
    let frame = raster(&script, 1.0);
    // Head (radius 20 about the origin): a 3-unit black band outside, skin inside.
    for (x, y) in [(21.5, 0.0), (-21.5, 0.0), (0.0, -21.5), (15.6, -15.6)] {
        assert!(black(at(&frame, x, y)), "head outline at ({x},{y})");
    }
    assert_eq!(
        at(&frame, 18.0, 0.0),
        SKIN,
        "behind mode leaves the fill edge"
    );
    assert_eq!(at(&frame, 24.5, 0.0), BACKGROUND);
    // The body's outline (top at y = 10) is covered by the later head fill...
    assert_eq!(at(&frame, 0.0, 9.5), SKIN);
    assert_eq!(at(&frame, 0.0, 11.5), SKIN);
    // ...while the head's own outline separates it from the body below.
    assert!(black(at(&frame, 0.0, 21.5)));
    assert_eq!(at(&frame, 0.0, 30.0), GREEN);
    // The body's outline still frames the exposed silhouette.
    assert!(black(at(&frame, 31.5, 30.0)) && black(at(&frame, 0.0, 51.5)));
}

#[test]
fn rounded_badge_has_no_polygon_corners_and_a_centred_outline() {
    let frame = raster(&script(), 1.0);
    // Control square -60..-30. Edge midpoints stay on the curve (centred 2-unit stroke).
    assert!(black(at(&frame, -45.0, -60.0)) && black(at(&frame, -60.0, -45.0)));
    assert_eq!(at(&frame, -45.0, -45.0), BLUE);
    // Every control corner (and the pixel just inside it) is cut away: background.
    for (x, y) in [
        (-60.0, -60.0),
        (-31.0, -60.0),
        (-31.0, -31.0),
        (-60.0, -31.0),
    ] {
        assert_eq!(at(&frame, x, y), BACKGROUND, "corner ({x},{y})");
    }
    for (x, y) in [
        (-58.0, -58.0),
        (-32.5, -57.5),
        (-32.5, -32.5),
        (-57.5, -32.5),
    ] {
        assert_eq!(at(&frame, x, y), BACKGROUND, "rounded corner ({x},{y})");
    }
    // The curve through the corner's B-spline midpoint is outlined (t = 1/2 at -56.25).
    assert!(black(at(&frame, -56.25, -56.25)));
}

#[test]
fn outlined_frames_are_deterministic_and_preview_matches_png() {
    let script = script();
    let preview = ScenePreview::new(
        Arc::new(script.clone()),
        0,
        Arc::new(ArtContext::default()),
        None,
    )
    .unwrap();
    let cancel = CancellationToken::new();
    let samples = [36_000u64, 0, 24_000, 12_000, 36_000];
    let forward: Vec<_> = samples
        .iter()
        .map(|s| preview.render(*s, 256, &cancel).unwrap().rgba)
        .collect();
    for (sample, frame) in samples.iter().zip(&forward).rev() {
        assert_eq!(&preview.render(*sample, 256, &cancel).unwrap().rgba, frame);
        assert_eq!(
            frame.as_slice(),
            raster(&script, *sample as f64 / 24_000.0).data(),
            "sample {sample}"
        );
    }
}
