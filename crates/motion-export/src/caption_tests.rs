//! Caption pixel and timing acceptance on the shared preview/SVG/PNG/sequence paths.
//! These tests establish determinism, geometry and colour only; they say nothing
//! about typographic or perceptual quality.
use super::*;
use motion_voice::export::DialogueManifest;
use resvg::tiny_skia::Pixmap;
use std::sync::Arc;

const W: u32 = 640;
const H: u32 = 360;

fn script_with(captions: &str, text_a: &str, text_b: &str) -> (Script, RenderedScene) {
    let source = format!(
        "project: {{fps: 4, width: {W}, height: {H}{captions}}}\ncharacters:\n  EMMA: {{asset: characters/emma}}\n  ALEX: {{asset: characters/alex}}\nscenes:\n  - id: intro\n    duration: 1.0\n    composition: {{background_color: '#000000'}}\n    events:\n      - {{type: speak, actor: EMMA, at: 0, text: '{text_a}', audio: a.wav}}\n      - {{type: speak, actor: ALEX, at: 0.5, text: '{text_b}', audio: b.wav}}\n"
    );
    let script = motion_script::parse(&source).unwrap();
    let (_, mut rendered) = crate::tests::fixture();
    rendered.manifest.dialogue = [("EMMA", 0u64), ("ALEX", 12000)]
        .into_iter()
        .map(|(actor, start_frame)| DialogueManifest {
            actor: actor.into(),
            start_frame,
            duration_frames: 12000,
            trim_start_frame: 0,
            gain: 1.0,
            alignment_source: None,
            visemes: vec![],
        })
        .collect();
    (script, rendered)
}

fn png_frame(script: &Script, rendered: &RenderedScene, frame: u64) -> Pixmap {
    let (_, _, png) = encode_frame(
        script,
        0,
        rendered,
        frame,
        true,
        &CancellationToken::new(),
        None,
    )
    .unwrap();
    Pixmap::decode_png(&png).unwrap()
}

fn lit_bounds(pixmap: &Pixmap, threshold: u8) -> Option<(u32, u32, u32, u32)> {
    let mut bounds: Option<(u32, u32, u32, u32)> = None;
    for y in 0..pixmap.height() {
        for x in 0..pixmap.width() {
            let p = pixmap.pixel(x, y).unwrap();
            if p.red().max(p.green()).max(p.blue()) > threshold {
                bounds = Some(match bounds {
                    None => (x, y, x, y),
                    Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x), y1.max(y)),
                });
            }
        }
    }
    bounds
}

fn count_where(pixmap: &Pixmap, f: impl Fn(u8, u8, u8) -> bool) -> usize {
    pixmap
        .pixels()
        .iter()
        .filter(|p| f(p.red(), p.green(), p.blue()))
        .count()
}

const PLAIN: &str = ", captions: {outline_width: 0}";

#[test]
fn the_font_database_holds_only_the_bundled_font() {
    let database = &media::options().fontdb;
    assert_eq!(database.len(), 1);
    let face = database.faces().next().unwrap();
    assert_eq!(face.families[0].0, motion_render::CAPTION_FONT_FAMILY);
}

#[test]
fn captions_are_opt_in_and_can_be_disabled() {
    let (none, rendered) = script_with("", "Hello there", "Hi");
    let (off, _) = script_with(", captions: {enabled: false}", "Hello there", "Hi");
    let (on, _) = script_with(PLAIN, "Hello there", "Hi");
    let base = png_frame(&none, &rendered, 0);
    // Opted out and explicitly disabled draw nothing: the black scene is untouched
    // (the actor label text is display:none, so it is not drawn either).
    assert_eq!(base.data(), png_frame(&off, &rendered, 0).data());
    assert!(lit_bounds(&base, 8).is_none() || lit_bounds(&base, 8).unwrap().1 < H / 2);
    let drawn = png_frame(&on, &rendered, 0);
    let changed = base
        .data()
        .iter()
        .zip(drawn.data())
        .filter(|(a, b)| a != b)
        .count();
    assert!(changed > 500, "text must change pixels, changed {changed}");
}

#[test]
fn fill_colour_follows_the_speaker_and_timing_follows_audio() {
    let captions =
        ", captions: {outline_width: 0, speaker_fill: {EMMA: '#ff0000', ALEX: '#00ff00'}}";
    let (script, rendered) = script_with(captions, "Hello there", "Hi you");
    let red = |r: u8, g: u8, b: u8| r > 200 && g < 40 && b < 40;
    let green = |r: u8, g: u8, b: u8| g > 200 && r < 40 && b < 40;
    // Frame 0 (t=0) is inside EMMA's clip, frame 2 (t=0.5) starts ALEX's.
    let first = png_frame(&script, &rendered, 0);
    let second = png_frame(&script, &rendered, 2);
    assert!(count_where(&first, red) > 300);
    assert_eq!(count_where(&first, green), 0);
    assert!(count_where(&second, green) > 300);
    assert_eq!(count_where(&second, red), 0);
    // The cue ends with the audio (0.5 s), so frame 1 (t=0.25) still shows EMMA
    // and the half-open interval hands over exactly at 0.5 s.
    assert!(count_where(&png_frame(&script, &rendered, 1), red) > 300);
    let cues = captions::cues_for_scene(&script, 0, Some(&rendered.manifest));
    assert_eq!(cues.len(), 2);
    assert!(cues.iter().all(|c| !c.estimated));
    assert_eq!((cues[0].start, cues[0].end), (0.0, 0.5));
    assert_eq!((cues[1].start, cues[1].end), (0.5, 1.0));
}

#[test]
fn unprepared_audio_is_estimated_and_marked() {
    let (script, _) = script_with(PLAIN, "Hello there", "Hi");
    let cues = captions::cues_for_scene(&script, 0, None);
    assert_eq!(cues.len(), 2);
    assert!(cues.iter().all(|c| c.estimated));
    assert_eq!(cues[0].end, captions::estimated_seconds("Hello there"));
    assert!(cues[1].end >= cues[1].start + 1.0);
}

#[test]
fn default_fill_applies_to_unlisted_speakers() {
    let captions = ", captions: {outline_width: 0, fill: '#0000ff'}";
    let (script, rendered) = script_with(captions, "Hello there", "Hi");
    let blue = |r: u8, g: u8, b: u8| b > 200 && r < 40 && g < 40;
    assert!(count_where(&png_frame(&script, &rendered, 0), blue) > 300);
}

#[test]
fn outline_is_drawn_in_its_own_colour() {
    let captions = ", captions: {outline: '#ff00ff', outline_width: 4}";
    let (script, rendered) = script_with(captions, "Hello there", "Hi");
    let magenta = |r: u8, g: u8, b: u8| r > 200 && b > 200 && g < 60;
    assert!(count_where(&png_frame(&script, &rendered, 0), magenta) > 200);
}

#[test]
fn wrapping_keeps_ink_inside_the_line_width_and_safe_margins() {
    let long = "Hey Stan what are you doing are you watching it right now";
    let captions =
        ", captions: {outline_width: 0, font_size: 40, max_width: 0.5, bottom_margin: 0.1}";
    let (script, rendered) = script_with(captions, long, "Hi");
    let style = script.project.captions.clone().unwrap();
    let lines = motion_render::captions::wrap_caption(&style, long, 0.5 * f64::from(W)).unwrap();
    assert!(lines.len() >= 3, "{lines:?}");
    let pixmap = png_frame(&script, &rendered, 0);
    let (x0, y0, x1, y1) = lit_bounds(&pixmap, 40).unwrap();
    // Measured widths ignore kerning (conservative), so ink fits the line width.
    assert!(
        x1 - x0 < (0.5 * f64::from(W)).ceil() as u32 + 2,
        "{x0} {x1}"
    );
    // Horizontally centred and inside the left/right safe margins.
    assert!(x0 >= (0.25 * f64::from(W)) as u32 - 2 && x1 <= (0.75 * f64::from(W)) as u32 + 2);
    assert!((x0 + x1) as i64 - i64::from(W) <= 4 && (i64::from(W) - (x0 + x1) as i64) <= 4);
    // Bottom ink stays above the bottom margin and the block is `lines` tall.
    assert!(y1 <= (0.9 * f64::from(H)).ceil() as u32 + 1, "{y1}");
    let pitch = 40.0 * style.line_height;
    assert!(f64::from(y1 - y0) > pitch * (lines.len() as f64 - 1.0) * 0.9);
    assert!(y0 > 0);
}

#[test]
fn the_same_inputs_give_identical_bytes_on_every_path() {
    let captions = ", captions: {speaker_fill: {EMMA: '#ffd400'}}";
    let (script, rendered) = script_with(captions, "Is that a caption?", "Yes it is");
    let cancel = CancellationToken::new();
    let (_, _, png) = encode_frame(&script, 0, &rendered, 0, true, &cancel, None).unwrap();
    let (_, _, again) = encode_frame(&script, 0, &rendered, 0, true, &cancel, None).unwrap();
    assert_eq!(png, again);
    // SVG export carries the captions and rasterizes to the exported PNG.
    let (_, _, svg) = encode_frame(&script, 0, &rendered, 0, false, &cancel, None).unwrap();
    let svg = String::from_utf8(svg).unwrap();
    assert!(svg.contains("id=\"captions\"") && svg.contains("IS THAT A CAPTION?"));
    assert_eq!(media::rasterize(&svg).unwrap(), png);
    // The interactive scene preview uses the same path.
    let expected = Pixmap::decode_png(&png).unwrap();
    let preview = ScenePreview::new(
        Arc::new(script.clone()),
        0,
        Arc::new(ArtContext::default()),
        Some(Arc::new(rendered.manifest.clone())),
    )
    .unwrap();
    assert_eq!(
        preview.render(0, 1024, &cancel).unwrap().rgba,
        expected.data()
    );
    // And so does the project sequence preview/export compositor.
    let sequence = render_sequence_frame(
        &script,
        std::slice::from_ref(&rendered),
        None,
        0,
        1024,
        &cancel,
    )
    .unwrap();
    assert_eq!(sequence.rgba, expected.data());
}

#[test]
fn missing_glyphs_are_reported_never_silently_dropped() {
    let (script, rendered) = script_with(PLAIN, "Hello \u{4f60}\u{597d}", "Hi");
    let cancel = CancellationToken::new();
    let error = encode_frame(&script, 0, &rendered, 0, true, &cancel, None)
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("U+4F60") && error.contains("bundled caption font"),
        "{error}"
    );
    let preview = ScenePreview::new(
        Arc::new(script.clone()),
        0,
        Arc::new(ArtContext::default()),
        None,
    );
    assert!(preview.is_err());
    assert!(captions::preflight(&script, 0, CaptionMode::Follow)
        .unwrap_err()
        .to_string()
        .contains("U+4F60"));
    // A disabled caption style does not reject the text.
    let (off, _) = script_with(", captions: {enabled: false}", "Hello \u{4f60}", "Hi");
    assert!(encode_frame(&off, 0, &rendered, 0, true, &cancel, None).is_ok());
}

#[test]
fn a_word_wider_than_the_line_is_an_error() {
    let captions = ", captions: {max_width: 0.1}";
    let (script, _) = script_with(captions, "Supercalifragilistic", "Hi");
    assert!(captions::preflight(&script, 0, CaptionMode::Follow)
        .unwrap_err()
        .to_string()
        .contains("wider than"));
}

// ---- user-level CaptionMode override --------------------------------------

fn mode_frame(script: &Script, rendered: &RenderedScene, mode: CaptionMode) -> Vec<u8> {
    let (_, _, png) = encode_frame_captions(
        script,
        0,
        rendered,
        0,
        true,
        &CancellationToken::new(),
        None,
        mode,
    )
    .unwrap();
    Pixmap::decode_png(&png).unwrap().take()
}

fn png_bytes(rgba: &[u8]) -> Vec<u8> {
    Pixmap::from_vec(
        rgba.to_vec(),
        resvg::tiny_skia::IntSize::from_wh(W, H).unwrap(),
    )
    .unwrap()
    .encode_png()
    .unwrap()
}

#[test]
fn caption_mode_show_hide_follow_change_pixels_over_every_screenplay_state() {
    let (on, rendered) = script_with(PLAIN, "Hello there", "Hi");
    let (off, _) = script_with(
        ", captions: {outline_width: 0, enabled: false}",
        "Hello there",
        "Hi",
    );
    let (none, _) = script_with("", "Hello there", "Hi");
    let blank = mode_frame(&none, &rendered, CaptionMode::Follow);
    let drawn = mode_frame(&on, &rendered, CaptionMode::Follow);
    assert_ne!(blank, drawn, "captions must change pixels");
    // Follow is exactly the screenplay.
    assert_eq!(mode_frame(&off, &rendered, CaptionMode::Follow), blank);
    // Hide removes captions even when the screenplay enables them.
    assert_eq!(mode_frame(&on, &rendered, CaptionMode::Hide), blank);
    assert_eq!(mode_frame(&off, &rendered, CaptionMode::Hide), blank);
    assert_eq!(mode_frame(&none, &rendered, CaptionMode::Hide), blank);
    // Show draws with the screenplay's style even when `enabled: false`.
    assert_eq!(mode_frame(&on, &rendered, CaptionMode::Show), drawn);
    assert_eq!(mode_frame(&off, &rendered, CaptionMode::Show), drawn);
}

#[test]
fn caption_mode_show_without_a_screenplay_style_is_an_error_not_a_silent_drop() {
    let (none, rendered) = script_with("", "Hello", "Hi");
    let cancel = CancellationToken::new();
    let frame = |script: &Script, mode| {
        encode_frame_captions(script, 0, &rendered, 0, true, &cancel, None, mode)
    };
    let error = frame(&none, CaptionMode::Show).unwrap_err().to_string();
    assert!(
        error.contains("project.captions") && error.contains("Show"),
        "{error}"
    );
    assert!(ScenePreview::new_with_captions(
        Arc::new(none.clone()),
        0,
        Arc::new(ArtContext::default()),
        None,
        CaptionMode::Show,
    )
    .is_err());
    // Missing glyphs also surface under Show even if the screenplay disabled captions.
    let (off, _) = script_with(", captions: {enabled: false}", "Hello \u{4f60}", "Hi");
    let error = frame(&off, CaptionMode::Show).unwrap_err().to_string();
    assert!(error.contains("U+4F60"), "{error}");
    // Hide never fails on text that cannot be drawn.
    assert!(frame(&off, CaptionMode::Hide).is_ok());
}

#[test]
fn caption_mode_is_identical_across_scene_preview_sequence_preview_and_all_exports() {
    let (script, rendered) = script_with(", captions: {enabled: false}", "Hello there", "Hi");
    let cancel = CancellationToken::new();
    let art = ArtContext::default();
    let scenes = vec![rendered.clone()];
    let source_before = serde_json::to_string(&script).unwrap();
    for mode in [CaptionMode::Follow, CaptionMode::Show, CaptionMode::Hide] {
        let expected = mode_frame(&script, &rendered, mode);
        // Scene preview.
        let preview = ScenePreview::new_with_captions(
            Arc::new(script.clone()),
            0,
            Arc::new(art.clone()),
            Some(Arc::new(rendered.manifest.clone())),
            mode,
        )
        .unwrap();
        assert_eq!(
            preview.render(0, 1024, &cancel).unwrap().rgba,
            expected,
            "{mode:?}"
        );
        // Sequence preview.
        let sequence = SequencePreview::new(
            Arc::new(script.clone()),
            Arc::new(scenes.clone()),
            None,
            &cancel,
        )
        .unwrap();
        assert_eq!(
            sequence
                .render_frame_captions(0, 1024, &cancel, mode)
                .unwrap()
                .rgba,
            expected,
            "{mode:?}"
        );
        // Scene PNG and SVG export.
        let dir = crate::tests::destination();
        export_png_scene_with_art_captions(
            &script,
            0,
            &rendered,
            &art,
            &dir,
            &cancel,
            |_| {},
            mode,
        )
        .unwrap();
        let png = Pixmap::load_png(dir.join("frame_000000.png")).unwrap();
        assert_eq!(png.data(), expected, "{mode:?}");
        fs::remove_dir_all(&dir).unwrap();
        let dir = crate::tests::destination();
        export_scene_with_art_captions(&script, 0, &rendered, &art, &dir, &cancel, |_| {}, mode)
            .unwrap();
        let svg = fs::read_to_string(dir.join("frame_000000.svg")).unwrap();
        assert_eq!(
            svg.contains("id=\"captions\""),
            mode == CaptionMode::Show,
            "{mode:?}"
        );
        assert_eq!(media::rasterize(&svg).unwrap(), png_bytes(&expected));
        fs::remove_dir_all(&dir).unwrap();
        // Sequence PNG and SVG export.
        let dir = crate::tests::destination();
        export_png_sequence_captions(&script, &scenes, None, &dir, &cancel, |_| {}, mode).unwrap();
        let png = Pixmap::load_png(dir.join("frame-000000.png")).unwrap();
        assert_eq!(png.data(), expected, "{mode:?}");
        fs::remove_dir_all(&dir).unwrap();
        let dir = crate::tests::destination();
        export_svg_sequence_captions(&script, &scenes, None, &dir, &cancel, |_| {}, mode).unwrap();
        let svg = fs::read_to_string(dir.join("frame-000000.svg")).unwrap();
        assert_eq!(
            svg.contains("id=\"scene0_captions\""),
            mode == CaptionMode::Show,
            "{mode:?}"
        );
        fs::remove_dir_all(&dir).unwrap();
    }
    // The override never touches the screenplay data it was applied to.
    assert_eq!(serde_json::to_string(&script).unwrap(), source_before);
}
