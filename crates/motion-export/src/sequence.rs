//! Bounded project preview and PNG/WAV/MP4 export on one shared sample clock.
use super::*;
use motion_scene::SequenceTimeline;
use std::sync::Arc;
#[derive(Serialize)]
struct Contribution {
    scene_index: usize,
    local_sample: u64,
    weight: f64,
}
#[derive(Serialize)]
struct Frame {
    frame: u64,
    sample_frame: u64,
    file: String,
    contributions: Vec<Contribution>,
}
#[derive(Serialize)]
struct Span {
    scene_index: usize,
    scene_id: String,
    start_sample: u64,
    duration_samples: u64,
    outgoing_overlap: u64,
}
#[derive(Serialize)]
struct Manifest {
    schema_version: u32,
    sample_rate: u32,
    fps: u32,
    width: u32,
    height: u32,
    total_samples: u64,
    audio_file: String,
    scenes: Vec<Span>,
    frames: Vec<Frame>,
}
fn image(
    script: &Script,
    rendered: &[RenderedScene],
    art: Option<&ArtContext>,
    index: usize,
    sample: u64,
    max_side: Option<u32>,
    validated: Option<&motion_scene::ValidatedScript>,
) -> Result<resvg::tiny_skia::Pixmap, Error> {
    let time = sample as f64 / f64::from(SAMPLE_RATE);
    let mut pose = match validated {
        Some(validated) => validated.evaluate(index, time),
        None => motion_scene::evaluate(script, index, time),
    }
    .map_err(|e| invalid(e.to_string()))?;
    let mouths = mouths_at(&rendered[index].manifest, sample);
    motion_scene::apply_speech(&mut pose, &mouths);
    let svg = match art {
        Some(a) => motion_render::render_svg_with_art(
            &pose,
            pose.width,
            pose.height,
            &mouths,
            &script.scenes[index].id,
            &a.document,
            &a.images,
        ),
        None => motion_render::render_svg_with_mouths(&pose, pose.width, pose.height, &mouths),
    }
    .map_err(|e| invalid(e.to_string()))?;
    resvg::tiny_skia::Pixmap::decode_png(&media::rasterize_scaled(&svg, max_side)?)
        .map_err(|e| invalid(e.to_string()))
}
fn preflight(
    script: &Script,
    rendered: &[RenderedScene],
    art: Option<&ArtContext>,
    export: bool,
) -> Result<(SequenceTimeline, u64), Error> {
    if rendered.len() != script.scenes.len() {
        return Err(invalid("one rendered scene required per scene"));
    }
    let timeline = SequenceTimeline::compile(script).map_err(|e| invalid(e.to_string()))?;
    let total_samples = timeline.total_samples();
    let total = total_samples
        .checked_mul(u64::from(script.project.fps))
        .and_then(|n| n.checked_add(u64::from(SAMPLE_RATE) - 1))
        .ok_or_else(|| invalid("frame count overflow"))?
        / u64::from(SAMPLE_RATE);
    let pixels = u64::from(script.project.width) * u64::from(script.project.height);
    if total == 0
        || (export && total > MAX_FRAMES)
        || pixels > 16_000_000
        || (export
            && pixels
                .checked_mul(total)
                .and_then(|n| n.checked_mul(2))
                .is_none_or(|n| n > 1_000_000_000))
    {
        return Err(invalid("sequence frame or pixel-work budget exceeded"));
    }
    let source_samples = timeline.spans().iter().try_fold(0u64, |n, s| {
        n.checked_add(s.duration_samples)
            .ok_or_else(|| invalid("source audio size overflow"))
    })?;
    if total_samples > 24_000 * 600 || source_samples > 24_000 * 1200 {
        return Err(invalid("sequence exceeds audio duration budget"));
    }
    let source_bytes = source_samples
        .checked_mul(4)
        .ok_or_else(|| invalid("source audio byte overflow"))?;
    // Conservative working set includes retained source/mixed PCM, two RGBA frames,
    // raster/PNG scratch and retained artwork encoding/decoding copies.
    let raster_pixels = if export {
        pixels
    } else {
        pixels.min(1024 * 1024)
    };
    let mut working = raster_pixels
        .checked_mul(24)
        .and_then(|n| total_samples.checked_mul(4).and_then(|a| n.checked_add(a)))
        .and_then(|n| n.checked_add(source_bytes))
        .ok_or_else(|| invalid("sequence working-set overflow"))?;
    if working > MAX_BYTES {
        return Err(invalid("sequence estimated working set exceeds 256 MiB"));
    }
    for (index, scene) in rendered.iter().enumerate() {
        validate_mode(script, index, scene, export)?;
    }
    if let Some(a) = art {
        a.document.validate().map_err(|e| invalid(e.to_string()))?;
        let mut geometry = 0u64;
        for layer in &a.document.layers {
            let scene = script
                .scenes
                .iter()
                .find(|s| s.id == layer.scene_id)
                .ok_or_else(|| invalid("unknown artwork scene"))?;
            if layer.active.is_some_and(|r| r.end > scene.duration) {
                return Err(invalid("artwork visibility exceeds scene"));
            }
            geometry = geometry
                .checked_add(match &layer.content {
                    motion_art::Content::Stroke { points, .. } => points.len() as u64,
                    _ => 1,
                })
                .ok_or_else(|| invalid("art geometry overflow"))?;
            if layer.visible {
                if let motion_art::Content::Image { asset_id, .. } = &layer.content {
                    if !a.images.contains_key(asset_id) {
                        return Err(invalid("missing future artwork image"));
                    }
                }
            }
        }
        if geometry
            .checked_mul(if export { total } else { 1 })
            .is_none_or(|n| n > 4_000_000)
        {
            return Err(invalid("artwork frame-work budget exceeded"));
        }
        let bytes = a.images.values().try_fold(0usize, |n, i| {
            n.checked_add(i.png().len())
                .ok_or_else(|| invalid("image byte overflow"))
        })?;
        let image_pixels = a.images.values().try_fold(0u64, |n, i| {
            n.checked_add(u64::from(i.width()) * u64::from(i.height()))
                .ok_or_else(|| invalid("image pixel overflow"))
        })?;
        working = working
            .checked_add(
                (bytes as u64)
                    .checked_mul(4)
                    .ok_or_else(|| invalid("art working-set overflow"))?,
            )
            .and_then(|n| image_pixels.checked_mul(8).and_then(|p| n.checked_add(p)))
            .ok_or_else(|| invalid("art working-set overflow"))?;
        if working > MAX_BYTES {
            return Err(invalid("sequence artwork working set exceeds 256 MiB"));
        }
        if bytes > 32 * 1024 * 1024 || image_pixels > 16_000_000 {
            return Err(invalid("aggregate artwork image budget exceeded"));
        }
    }
    Ok((timeline, total))
}
#[allow(clippy::too_many_arguments)]
fn compose(
    script: &Script,
    rendered: &[RenderedScene],
    art: Option<&ArtContext>,
    timeline: &SequenceTimeline,
    sample: u64,
    max_side: Option<u32>,
    cancel: &CancellationToken,
    validated: Option<&motion_scene::ValidatedScript>,
) -> Result<(resvg::tiny_skia::Pixmap, Vec<Contribution>), Error> {
    let contributions = timeline
        .sample(sample)
        .map_err(|e| invalid(e.to_string()))?;
    if contributions.is_empty() || contributions.len() > 2 {
        return Err(invalid("invalid sequence contributions"));
    }
    let mut images = Vec::with_capacity(2);
    let mut entries = Vec::new();
    for c in contributions {
        check(cancel)?;
        let src = image(
            script,
            rendered,
            art,
            c.scene_index,
            c.local_sample,
            max_side,
            validated,
        )?;
        if src.data().as_chunks::<4>().0.iter().any(|p| p[3] != 255) {
            return Err(invalid(
                "sequence frames must be complete opaque RGBA images",
            ));
        }
        images.push(src);
        entries.push(Contribution {
            scene_index: c.scene_index,
            local_sample: c.local_sample,
            weight: c.weight,
        });
    }
    let mut out = images.remove(0);
    if let Some(incoming) = images.first() {
        for (value, next) in out.data_mut().iter_mut().zip(incoming.data()) {
            *value = (f64::from(*value) * entries[0].weight + f64::from(*next) * entries[1].weight)
                .round()
                .clamp(0.0, 255.0) as u8;
        }
    }
    Ok((out, entries))
}
/// A complete RGBA project frame, composed with the export renderer.
#[derive(Debug, Clone)]
pub struct SequencePreviewFrame {
    pub sample_frame: u64,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}
/// Immutable, resource-validated project data shared by all preview requests.
/// Construct on a worker once after preparing audio; retained PCM is shared by Arc.
pub struct SequencePreview {
    script: Arc<Script>,
    rendered: Arc<Vec<RenderedScene>>,
    art: Option<Arc<ArtContext>>,
    timeline: SequenceTimeline,
    validated: motion_scene::ValidatedScript,
}
impl SequencePreview {
    pub fn new(
        script: Arc<Script>,
        rendered: Arc<Vec<RenderedScene>>,
        art: Option<Arc<ArtContext>>,
        cancel: &CancellationToken,
    ) -> Result<Self, Error> {
        check(cancel)?;
        let (timeline, _) = preflight(&script, &rendered, art.as_deref(), false)?;
        check(cancel)?;
        let validated = motion_scene::ValidatedScript::new(script.clone())
            .map_err(|e| invalid(e.to_string()))?;
        Ok(Self {
            script,
            rendered,
            art,
            timeline,
            validated,
        })
    }
    pub fn render_frame(
        &self,
        sample: u64,
        max_side: u32,
        cancel: &CancellationToken,
    ) -> Result<SequencePreviewFrame, Error> {
        preview_frame(
            &self.script,
            &self.rendered,
            self.art.as_deref(),
            &self.timeline,
            sample,
            max_side,
            cancel,
            Some(&self.validated),
        )
    }
}
#[allow(clippy::too_many_arguments)]
fn preview_frame(
    script: &Script,
    rendered: &[RenderedScene],
    art: Option<&ArtContext>,
    timeline: &SequenceTimeline,
    global_sample: u64,
    max_side: u32,
    cancel: &CancellationToken,
    validated: Option<&motion_scene::ValidatedScript>,
) -> Result<SequencePreviewFrame, Error> {
    check(cancel)?;
    if !(1..=1024).contains(&max_side) {
        return Err(invalid("preview maximum side must be 1..1024"));
    }
    let (image, _) = compose(
        script,
        rendered,
        art,
        timeline,
        global_sample,
        Some(max_side),
        cancel,
        validated,
    )?;
    check(cancel)?;
    Ok(SequencePreviewFrame {
        sample_frame: global_sample,
        width: image.width(),
        height: image.height(),
        rgba: image.data().to_vec(),
    })
}
/// Render a bounded preview without changing scene coordinates or starting synthesis.
pub fn render_sequence_frame(
    script: &Script,
    rendered: &[RenderedScene],
    art: Option<&ArtContext>,
    global_sample: u64,
    max_side: u32,
    cancel: &CancellationToken,
) -> Result<SequencePreviewFrame, Error> {
    check(cancel)?;
    if !(1..=1024).contains(&max_side) {
        return Err(invalid("preview maximum side must be 1..1024"));
    }
    let (timeline, _) = preflight(script, rendered, art, false)?;
    preview_frame(
        script,
        rendered,
        art,
        &timeline,
        global_sample,
        max_side,
        cancel,
        None,
    )
}
/// Export the same PNG/WAV composition and encode it with an explicit local FFmpeg.
#[allow(clippy::too_many_arguments)]
pub fn export_mp4_sequence(
    script: &Script,
    rendered: &[RenderedScene],
    art: Option<&ArtContext>,
    destination: &Path,
    ffmpeg: &Path,
    cancel: &CancellationToken,
    progress: impl FnMut(Progress),
) -> Result<Report, Error> {
    check(cancel)?;
    media::validate_encoder(script, ffmpeg)?;
    let report = export_png_sequence(script, rendered, art, destination, cancel, progress)?;
    let result = media::encode_named(
        script,
        destination,
        ffmpeg,
        cancel,
        "frame-%06d.png",
        "audio.wav",
        "sequence.mp4",
        "sequence.json",
    )
    .map(|()| report);
    if result.is_err() {
        fs::remove_dir_all(destination)
            .map_err(|e| invalid(format!("partial-output cleanup failed: {e}")))?;
    }
    result
}
/// Blocking sequential PNG/WAV export on the shared sample clock. Complete opaque
/// images are mixed by channel sums, never by repeated source-over.
pub fn export_png_sequence(
    script: &Script,
    rendered: &[RenderedScene],
    art: Option<&ArtContext>,
    destination: &Path,
    cancel: &CancellationToken,
    mut progress: impl FnMut(Progress),
) -> Result<Report, Error> {
    check(cancel)?;
    let (timeline, total) = preflight(script, rendered, art, true)?;
    let total_samples = timeline.total_samples();
    for index in 0..rendered.len() {
        check(cancel)?;
        image(script, rendered, art, index, 0, None, None)?;
    }
    let audio = motion_voice::sequence::mix_sequence(&timeline, rendered, cancel).map_err(|e| {
        if cancel.is_cancelled() {
            Error::Cancelled
        } else {
            invalid(e.to_string())
        }
    })?;
    check(cancel)?;
    if destination.exists() {
        return Err(invalid("destination already exists"));
    }
    fs::create_dir(destination)?;
    let result = (|| {
        let mut used = 44u64
            .checked_add(
                (audio.samples.len() as u64)
                    .checked_mul(2)
                    .ok_or_else(|| invalid("audio size overflow"))?,
            )
            .ok_or_else(|| invalid("audio size overflow"))?;
        if used > MAX_BYTES {
            return Err(invalid("sequence output byte budget exceeded"));
        }
        motion_audio::write_wav(destination.join("audio.wav"), &audio)
            .map_err(|e| invalid(e.to_string()))?;
        let mut frames = Vec::new();
        for frame in 0..total {
            check(cancel)?;
            let sample = frame
                .checked_mul(u64::from(SAMPLE_RATE))
                .ok_or_else(|| invalid("frame clock overflow"))?
                / u64::from(script.project.fps);
            let (out, entries) =
                compose(script, rendered, art, &timeline, sample, None, cancel, None)?;
            let bytes = out.encode_png().map_err(|e| invalid(e.to_string()))?;
            used = used
                .checked_add(bytes.len() as u64)
                .ok_or_else(|| invalid("output byte overflow"))?;
            if used > MAX_BYTES {
                return Err(invalid("sequence output byte budget exceeded"));
            }
            let file = format!("frame-{frame:06}.png");
            write_new(&destination.join(&file), &bytes)?;
            frames.push(Frame {
                frame,
                sample_frame: sample,
                file,
                contributions: entries,
            });
            progress(Progress {
                completed_frames: frame + 1,
                total_frames: total,
            });
        }
        check(cancel)?;
        let manifest = bounded_json(
            &Manifest {
                schema_version: 1,
                sample_rate: SAMPLE_RATE,
                fps: script.project.fps,
                width: script.project.width,
                height: script.project.height,
                total_samples,
                audio_file: "audio.wav".into(),
                scenes: timeline
                    .spans()
                    .iter()
                    .map(|s| Span {
                        scene_index: s.scene_index,
                        scene_id: s.scene_id.clone(),
                        start_sample: s.start_sample,
                        duration_samples: s.duration_samples,
                        outgoing_overlap: s.outgoing_overlap,
                    })
                    .collect(),
                frames,
            },
            8 * 1024 * 1024,
        )?;
        if used
            .checked_add(manifest.len() as u64)
            .is_none_or(|n| n > MAX_BYTES)
        {
            return Err(invalid("sequence output byte budget exceeded"));
        }
        write_new(&destination.join("sequence.json"), &manifest)?;
        Ok(Report {
            frame_count: total,
            output_directory: destination.to_path_buf(),
        })
    })();
    if result.is_err() {
        fs::remove_dir_all(destination)
            .map_err(|e| invalid(format!("partial-output cleanup failed: {e}")))?;
    }
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    fn fixture(fade: bool) -> (Script, Vec<RenderedScene>) {
        let (mut script, mut first) = crate::tests::fixture();
        script.project.width = 8;
        script.project.height = 8;
        script.characters.clear();
        first.manifest.dialogue.clear();
        script.scenes[0].composition.background_color = "#ff0000".into();
        let mut second = script.scenes[0].clone();
        second.id = "second".into();
        second.composition.background_color = "#0000ff".into();
        if fade {
            script.scenes[0].composition.transition =
                motion_script::Transition::Crossfade { duration: 0.5 };
        }
        script.scenes.push(second);
        let second = RenderedScene {
            audio: first.audio.clone(),
            manifest: SceneManifest {
                scene_id: "second".into(),
                ..first.manifest.clone()
            },
        };
        (script, vec![first, second])
    }
    fn pixel(path: &Path, frame: u64) -> [u8; 4] {
        let png = fs::read(path.join(format!("frame-{frame:06}.png"))).unwrap();
        let pix = resvg::tiny_skia::Pixmap::decode_png(&png).unwrap();
        pix.data()[0..4].try_into().unwrap()
    }
    #[test]
    fn preview_matches_export_pixels_and_scales_original_coordinates() {
        let (script, rendered) = fixture(true);
        let dest = crate::tests::destination();
        export_png_sequence(
            &script,
            &rendered,
            None,
            &dest,
            &CancellationToken::new(),
            |_| {},
        )
        .unwrap();
        for frame in 0..6 {
            let preview = render_sequence_frame(
                &script,
                &rendered,
                None,
                frame * 6000,
                8,
                &CancellationToken::new(),
            )
            .unwrap();
            let png = resvg::tiny_skia::Pixmap::decode_png(
                &fs::read(dest.join(format!("frame-{frame:06}.png"))).unwrap(),
            )
            .unwrap();
            assert_eq!(preview.rgba, png.data());
            assert_eq!((preview.width, preview.height), (8, 8));
        }
        let small = render_sequence_frame(
            &script,
            &rendered,
            None,
            18000,
            4,
            &CancellationToken::new(),
        )
        .unwrap();
        assert_eq!((small.width, small.height), (4, 4));
        assert_eq!(&small.rgba[0..4], &[128, 0, 128, 255]);
        assert!(
            render_sequence_frame(&script, &rendered, None, 0, 0, &CancellationToken::new())
                .is_err()
        );
        assert!(render_sequence_frame(
            &script,
            &rendered,
            None,
            36000,
            8,
            &CancellationToken::new()
        )
        .is_err());
        fs::remove_dir_all(dest).unwrap();
    }
    #[test]
    fn prepared_hd_preview_is_independent_of_export_work_budget() {
        let (mut script, _) = fixture(false);
        script.scenes.truncate(1);
        script.project.width = 1920;
        script.project.height = 1080;
        script.project.fps = 30;
        script.scenes[0].duration = 30.0;
        let rendered =
            vec![motion_voice::export::render_scene(&script.scenes[0].id, 30.0, &[]).unwrap()];
        let cancel = CancellationToken::new();
        let dest = crate::tests::destination();
        assert!(export_png_sequence(&script, &rendered, None, &dest, &cancel, |_| {}).is_err());
        assert!(!dest.exists());
        let preview = SequencePreview::new(
            Arc::new(script.clone()),
            Arc::new(rendered.clone()),
            None,
            &cancel,
        )
        .unwrap();
        // Repeated rendering uses immutable validated data, with no export-wide
        // resource scan or first-frame rasterization of unrelated scenes.
        for sample in [0, 24_000, 719_999] {
            let frame = preview.render_frame(sample, 320, &cancel).unwrap();
            assert_eq!((frame.width, frame.height), (320, 180));
            assert_eq!(frame.rgba.len(), 320 * 180 * 4);
        }
        assert!(render_sequence_frame(&script, &rendered, None, 0, 320, &cancel).is_ok());
        assert!(preview.render_frame(720_000, 320, &cancel).is_err());
        cancel.cancel();
        assert!(matches!(
            preview.render_frame(0, 320, &cancel),
            Err(Error::Cancelled)
        ));
    }
    #[test]
    fn mp4_encoder_and_cancel_preflight_leave_no_output() {
        let (script, rendered) = fixture(true);
        let dest = crate::tests::destination();
        assert!(export_mp4_sequence(
            &script,
            &rendered,
            None,
            &dest,
            Path::new("ffmpeg"),
            &CancellationToken::new(),
            |_| {}
        )
        .is_err());
        assert!(!dest.exists());
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            export_mp4_sequence(
                &script,
                &rendered,
                None,
                &dest,
                Path::new("ffmpeg"),
                &cancel,
                |_| {}
            ),
            Err(Error::Cancelled)
        ));
        assert!(!dest.exists());
    }
    #[test]
    #[ignore = "requires explicit REAL_FFMPEG absolute local encoder"]
    fn real_ffmpeg_sequence_has_duration_and_crossfade_color() {
        use std::process::Command;
        let ffmpeg = std::path::PathBuf::from(std::env::var("REAL_FFMPEG").expect("REAL_FFMPEG"));
        let (script, rendered) = fixture(true);
        let dest = crate::tests::destination();
        export_mp4_sequence(
            &script,
            &rendered,
            None,
            &dest,
            &ffmpeg,
            &CancellationToken::new(),
            |_| {},
        )
        .unwrap();
        let out = Command::new(&ffmpeg)
            .args(["-v", "error", "-i"])
            .arg(dest.join("sequence.mp4"))
            .args([
                "-vf",
                "select=eq(n\\,3)",
                "-frames:v",
                "1",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgb24",
                "pipe:1",
            ])
            .output()
            .unwrap();
        assert!(out.status.success());
        let rgb = &out.stdout[0..3];
        assert!((100..155).contains(&rgb[0]) && rgb[1] < 25 && (100..155).contains(&rgb[2]));
        let pcm = Command::new(&ffmpeg)
            .args(["-v", "error", "-i"])
            .arg(dest.join("sequence.mp4"))
            .args(["-vn", "-ar", "24000", "-ac", "1", "-f", "s16le", "pipe:1"])
            .output()
            .unwrap();
        assert!(pcm.status.success());
        let duration = pcm.stdout.len() as f64 / 48000.0;
        assert!((duration - 1.5).abs() < 0.1, "duration {duration}");
        fs::remove_dir_all(dest).unwrap();
    }
    #[test]
    fn decoded_cut_crossfade_and_exact_sample_manifest() {
        for fade in [false, true] {
            let (script, rendered) = fixture(fade);
            let dest = crate::tests::destination();
            let report = export_png_sequence(
                &script,
                &rendered,
                None,
                &dest,
                &CancellationToken::new(),
                |_| {},
            )
            .unwrap();
            assert_eq!(pixel(&dest, 0), [255, 0, 0, 255]);
            if fade {
                assert_eq!(report.frame_count, 6);
                assert_eq!(pixel(&dest, 2), [255, 0, 0, 255]);
                assert_eq!(pixel(&dest, 3), [128, 0, 128, 255]);
                assert_eq!(pixel(&dest, 4), [0, 0, 255, 255]);
            } else {
                assert_eq!(report.frame_count, 8);
                assert_eq!(pixel(&dest, 3), [255, 0, 0, 255]);
                assert_eq!(pixel(&dest, 4), [0, 0, 255, 255]);
            }
            let m: serde_json::Value =
                serde_json::from_slice(&fs::read(dest.join("sequence.json")).unwrap()).unwrap();
            assert_eq!(m["frames"][3]["sample_frame"], 18000);
            if fade {
                assert_eq!(m["frames"][3]["contributions"][1]["local_sample"], 6000);
            }
            assert_eq!(
                motion_audio::read_wav(dest.join("audio.wav"))
                    .unwrap()
                    .frames() as u64,
                if fade { 36000 } else { 48000 }
            );
            fs::remove_dir_all(&dest).unwrap();
        }
    }
    #[test]
    fn timed_art_is_composited_before_complete_frame_crossfade() {
        use motion_art::{Color, Content, Layer, Plane, Rect, TimeRange};
        let (script, rendered) = fixture(true);
        let mut art = ArtContext::default();
        art.document.layers.push(Layer {
            id: "timed".into(),
            scene_id: "second".into(),
            name: "timed".into(),
            visible: true,
            plane: Plane::Foreground,
            active: Some(TimeRange {
                start: 0.25,
                end: 0.5,
            }),
            opacity: 1.0,
            attachment: None,
            clip: None,
            content: Content::Rectangle {
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 8.0,
                    height: 8.0,
                },
                color: Color {
                    r: 0,
                    g: 255,
                    b: 0,
                    a: 255,
                },
            },
        });
        let dest = crate::tests::destination();
        export_png_sequence(
            &script,
            &rendered,
            Some(&art),
            &dest,
            &CancellationToken::new(),
            |_| {},
        )
        .unwrap();
        assert_eq!(pixel(&dest, 2), [255, 0, 0, 255]);
        assert_eq!(pixel(&dest, 3), [128, 128, 0, 255]);
        let preview = render_sequence_frame(
            &script,
            &rendered,
            Some(&art),
            18000,
            8,
            &CancellationToken::new(),
        )
        .unwrap();
        let exported =
            resvg::tiny_skia::Pixmap::decode_png(&fs::read(dest.join("frame-000003.png")).unwrap())
                .unwrap();
        assert_eq!(preview.rgba, exported.data());
        assert_eq!(pixel(&dest, 4), [0, 0, 255, 255]);
        fs::remove_dir_all(&dest).unwrap();
        art.document.layers[0].active = Some(TimeRange {
            start: 1.0,
            end: 2.0,
        });
        assert!(export_png_sequence(
            &script,
            &rendered,
            Some(&art),
            &dest,
            &CancellationToken::new(),
            |_| {}
        )
        .is_err());
        assert!(!dest.exists());
    }
    #[test]
    fn cancellation_removes_only_owned_output_and_preserves_existing() {
        let (script, rendered) = fixture(true);
        let dest = crate::tests::destination();
        let token = CancellationToken::new();
        let result =
            export_png_sequence(&script, &rendered, None, &dest, &token, |_| token.cancel());
        assert!(matches!(result, Err(Error::Cancelled)));
        assert!(!dest.exists());
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("keep"), b"keep").unwrap();
        assert!(export_png_sequence(
            &script,
            &rendered,
            None,
            &dest,
            &CancellationToken::new(),
            |_| {}
        )
        .is_err());
        assert!(dest.join("keep").exists());
        fs::remove_dir_all(&dest).unwrap();
    }
    #[test]
    fn future_scene_audio_and_resource_limits_preflight_before_creation() {
        let (mut script, mut rendered) = fixture(false);
        let dest = crate::tests::destination();
        rendered[1].manifest.scene_id = "wrong".into();
        assert!(export_png_sequence(
            &script,
            &rendered,
            None,
            &dest,
            &CancellationToken::new(),
            |_| {}
        )
        .is_err());
        assert!(!dest.exists());
        rendered[1].manifest.scene_id = "second".into();
        script.project.width = 16384;
        script.project.height = 16384;
        assert!(export_png_sequence(
            &script,
            &rendered,
            None,
            &dest,
            &CancellationToken::new(),
            |_| {}
        )
        .is_err());
        assert!(!dest.exists());
    }
}
