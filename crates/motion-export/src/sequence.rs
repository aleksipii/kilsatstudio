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
fn scene_svg(
    script: &Script,
    rendered: &[RenderedScene],
    art: Option<&ArtContext>,
    index: usize,
    sample: u64,
    validated: Option<&motion_scene::ValidatedScript>,
) -> Result<String, Error> {
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
    Ok(svg)
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
    let svg = scene_svg(script, rendered, art, index, sample, validated)?;
    resvg::tiny_skia::Pixmap::decode_png(&media::rasterize_scaled(&svg, max_side)?)
        .map_err(|e| invalid(e.to_string()))
}

/// Prefix references in internally generated SVG only. Prefixing every ID,
/// rather than a list of today's clip names, also protects future definitions.
fn namespace_svg(svg: &str, prefix: &str) -> String {
    // Only internally generated XML is accepted here: attribute values/text
    // escape literal angle brackets. Keep visible labels untouched and make
    // three linear passes per tag, including large embedded image tags.
    let mut out = String::with_capacity(svg.len());
    for segment in svg.split_inclusive('>') {
        if let Some(start) = segment.find('<') {
            out.push_str(&segment[..start]);
            let tag = segment[start..]
                .replace("id=\"", &format!("id=\"{prefix}"))
                .replace("url(#", &format!("url(#{prefix}"))
                .replace("href=\"#", &format!("href=\"#{prefix}"));
            out.push_str(&tag);
        } else {
            out.push_str(segment);
        }
    }
    out
}

const MAX_SVG_FRAME_BYTES: usize = 64 * 1024 * 1024;

fn compose_svg(
    script: &Script,
    rendered: &[RenderedScene],
    art: Option<&ArtContext>,
    timeline: &SequenceTimeline,
    sample: u64,
    cancel: &CancellationToken,
    validated: Option<&motion_scene::ValidatedScript>,
) -> Result<(Vec<u8>, Vec<Contribution>), Error> {
    let contributions = timeline
        .sample(sample)
        .map_err(|e| invalid(e.to_string()))?;
    if contributions.is_empty() || contributions.len() > 2 {
        return Err(invalid("invalid sequence contributions"));
    }
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{}\" height=\"{}\" viewBox=\"0 0 {} {}\">",
        script.project.width, script.project.height, script.project.width, script.project.height
    );
    let mut entries = Vec::with_capacity(2);
    for (slot, c) in contributions.into_iter().enumerate() {
        check(cancel)?;
        let svg = scene_svg(
            script,
            rendered,
            art,
            c.scene_index,
            c.local_sample,
            validated,
        )?;
        if svg.len() > MAX_SVG_FRAME_BYTES {
            return Err(invalid("sequence SVG scene byte budget exceeded"));
        }
        let svg = namespace_svg(&svg, &format!("scene{slot}_"));
        // Each generated scene includes an opaque black full-canvas backdrop.
        // Outgoing stays opaque; only the isolated incoming scene gets weighted.
        // This gives the same linear channel mix as the PNG compositor.
        let opacity = if slot == 0 { 1.0 } else { c.weight };
        let group = format!("<g opacity=\"{opacity:.12}\">{svg}</g>");
        if out
            .len()
            .checked_add(group.len())
            .and_then(|n| n.checked_add("</svg>".len()))
            .is_none_or(|n| n > MAX_SVG_FRAME_BYTES)
        {
            return Err(invalid("sequence SVG byte budget exceeded"));
        }
        out.push_str(&group);
        entries.push(Contribution {
            scene_index: c.scene_index,
            local_sample: c.local_sample,
            weight: c.weight,
        });
    }
    check(cancel)?;
    out.push_str("</svg>");
    Ok((out.into_bytes(), entries))
}
// Rendering embeds image data once per layer, not once per unique asset.
// Include inactive/invisible occurrences conservatively before generating SVG.
fn embedded_image_budget(art: &ArtContext, limit: u64) -> Result<u64, Error> {
    let mut scenes = std::collections::BTreeMap::<&str, u64>::new();
    for layer in &art.document.layers {
        if let motion_art::Content::Image { asset_id, .. } = &layer.content {
            if let Some(image) = art.images.get(asset_id) {
                let encoded = (image.png().len() as u64)
                    .checked_add(2)
                    .and_then(|n| (n / 3).checked_mul(4))
                    .and_then(|n| n.checked_add(256))
                    .ok_or_else(|| invalid("embedded image byte overflow"))?;
                let total = scenes.entry(&layer.scene_id).or_default();
                *total = total
                    .checked_add(encoded)
                    .ok_or_else(|| invalid("embedded image byte overflow"))?;
                if *total > limit {
                    return Err(invalid("per-scene embedded image byte budget exceeded"));
                }
            }
        }
    }
    Ok(scenes.values().copied().max().unwrap_or(0))
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
        let embedded = embedded_image_budget(a, 32 * 1024 * 1024)?;
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
            if let Some(motion_art::ClipShape::Polygon { points }) = &layer.clip_shape {
                geometry = geometry
                    .checked_add(points.len() as u64)
                    .ok_or_else(|| invalid("mask geometry overflow"))?;
            }
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
            // Conservative encoding scratch estimate, including two scene
            // contributions, namespacing replacement and nested group copies.
            // Allocator overhead/vector text remain estimates, not a heap cap.
            .and_then(|n| embedded.checked_mul(6).and_then(|p| n.checked_add(p)))
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
    progress: impl FnMut(Progress),
) -> Result<Report, Error> {
    export_project_sequence(script, rendered, art, destination, cancel, progress, false)
}

/// Export editable vector frames plus mixed WAV and sample-accurate sequence manifest.
/// Imported PNG artwork remains embedded; generated scene geometry stays vector.
pub fn export_svg_sequence(
    script: &Script,
    rendered: &[RenderedScene],
    art: Option<&ArtContext>,
    destination: &Path,
    cancel: &CancellationToken,
    progress: impl FnMut(Progress),
) -> Result<Report, Error> {
    export_project_sequence(script, rendered, art, destination, cancel, progress, true)
}

fn export_project_sequence(
    script: &Script,
    rendered: &[RenderedScene],
    art: Option<&ArtContext>,
    destination: &Path,
    cancel: &CancellationToken,
    mut progress: impl FnMut(Progress),
    svg: bool,
) -> Result<Report, Error> {
    check(cancel)?;
    let (timeline, total) = preflight(script, rendered, art, true)?;
    let validated = motion_scene::ValidatedScript::new(Arc::new(script.clone()))
        .map_err(|e| invalid(e.to_string()))?;
    let total_samples = timeline.total_samples();
    for index in 0..rendered.len() {
        check(cancel)?;
        image(script, rendered, art, index, 0, None, Some(&validated))?;
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
            let (bytes, entries) = if svg {
                compose_svg(
                    script,
                    rendered,
                    art,
                    &timeline,
                    sample,
                    cancel,
                    Some(&validated),
                )?
            } else {
                let (out, entries) = compose(
                    script,
                    rendered,
                    art,
                    &timeline,
                    sample,
                    None,
                    cancel,
                    Some(&validated),
                )?;
                (
                    out.encode_png().map_err(|e| invalid(e.to_string()))?,
                    entries,
                )
            };
            used = used
                .checked_add(bytes.len() as u64)
                .ok_or_else(|| invalid("output byte overflow"))?;
            if used > MAX_BYTES {
                return Err(invalid("sequence output byte budget exceeded"));
            }
            let file = format!("frame-{frame:06}.{}", if svg { "svg" } else { "png" });
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
    fn vector_sequence_raster_matches_png_cuts_crossfades_and_mask_namespaces() {
        use motion_art::{BlendMode, ClipShape, Color, Content, Layer, Plane, Rect};
        for fade in [false, true] {
            let (mut script, rendered) = fixture(fade);
            for scene in &mut script.scenes {
                scene.composition.fade_in = 0.25;
                scene.composition.fade_out = 0.25;
                scene.composition.camera = vec![
                    motion_script::CameraKey {
                        at: 0.0,
                        x: 0.0,
                        y: 0.0,
                        zoom: 1.0,
                    },
                    motion_script::CameraKey {
                        at: 1.0,
                        x: 1.0,
                        y: 0.5,
                        zoom: 1.2,
                    },
                ];
            }
            let mut art = ArtContext::default();
            for (index, scene) in script.scenes.iter().enumerate() {
                art.document.layers.push(Layer {
                    id: format!("mask{index}"),
                    scene_id: scene.id.clone(),
                    name: "masked".into(),
                    visible: true,
                    plane: Plane::Foreground,
                    active: None,
                    opacity: 0.7,
                    attachment: None,
                    clip: if index == 0 {
                        Some(Rect {
                            x: 0.0,
                            y: 0.0,
                            width: 6.0,
                            height: 8.0,
                        })
                    } else {
                        None
                    },
                    clip_shape: if index == 1 {
                        Some(ClipShape::Ellipse {
                            rect: Rect {
                                x: 0.0,
                                y: 0.0,
                                width: 8.0,
                                height: 8.0,
                            },
                        })
                    } else {
                        None
                    },
                    blend: BlendMode::Multiply,
                    content: Content::Rectangle {
                        rect: Rect {
                            x: 0.0,
                            y: 0.0,
                            width: 8.0,
                            height: 8.0,
                        },
                        color: Color {
                            r: 128,
                            g: 192,
                            b: 96,
                            a: 255,
                        },
                    },
                });
            }
            let mut image_layer = art.document.layers[1].clone();
            image_layer.id = "embedded".into();
            image_layer.clip_shape = None;
            image_layer.blend = BlendMode::Normal;
            image_layer.content = Content::Image {
                asset_id: "pixel".into(),
                rect: Rect {
                    x: 2.0,
                    y: 2.0,
                    width: 4.0,
                    height: 4.0,
                },
            };
            art.document.layers.push(image_layer);
            art.images.insert(
                "pixel".into(),
                motion_art::ImageResource::from_rgba(1, 1, vec![255, 255, 255, 255]).unwrap(),
            );
            let dest = crate::tests::destination();
            // A resource used only in the future scene must fail before output.
            let image = art.images.remove("pixel").unwrap();
            assert!(export_svg_sequence(
                &script,
                &rendered,
                Some(&art),
                &dest,
                &CancellationToken::new(),
                |_| {}
            )
            .is_err());
            assert!(!dest.exists());
            art.images.insert("pixel".into(), image);
            let cancel = CancellationToken::new();
            let report =
                export_svg_sequence(&script, &rendered, Some(&art), &dest, &cancel, |_| {})
                    .unwrap();
            let timeline = SequenceTimeline::compile(&script).unwrap();
            for frame in 0..report.frame_count {
                let sample = frame * 6000;
                let svg = fs::read_to_string(dest.join(format!("frame-{frame:06}.svg"))).unwrap();
                assert!(svg.contains("<rect"));
                if sample >= 24000 || (fade && sample >= 12000) {
                    assert!(svg.contains("data:image/png"));
                }
                assert!(!svg.contains("id=\"canvas\""));
                assert!(svg.contains("id=\"scene0_canvas\""));
                let raster = resvg::tiny_skia::Pixmap::decode_png(
                    &media::rasterize_scaled(&svg, None).unwrap(),
                )
                .unwrap();
                let (png, _) = compose(
                    &script,
                    &rendered,
                    Some(&art),
                    &timeline,
                    sample,
                    None,
                    &cancel,
                    None,
                )
                .unwrap();
                // SVG source-over quantizes premultiplied channels; PNG sums round once.
                for (a, b) in raster.data().iter().zip(png.data()) {
                    assert!(a.abs_diff(*b) <= 2, "frame {frame}: {a} != {b}");
                }
            }
            let manifest: serde_json::Value =
                serde_json::from_slice(&fs::read(dest.join("sequence.json")).unwrap()).unwrap();
            assert_eq!(manifest["frames"][3]["sample_frame"], 18000);
            assert_eq!(manifest["frames"][0]["file"], "frame-000000.svg");
            assert_eq!(
                motion_audio::read_wav(dest.join("audio.wav"))
                    .unwrap()
                    .frames() as u64,
                timeline.total_samples()
            );
            fs::remove_dir_all(dest).unwrap();
        }
    }

    #[test]
    fn repeated_image_occurrences_are_bounded_before_render_or_destination() {
        let (script, rendered) = fixture(false);
        let mut art = ArtContext::default();
        // Deterministic incompressible pixels keep this fixture small while
        // repeated references would expand into tens of MiB of SVG text.
        let mut state = 1u32;
        let rgba: Vec<u8> = (0..128 * 128 * 4)
            .map(|_| {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                state as u8
            })
            .collect();
        art.images.insert(
            "pixel".into(),
            motion_art::ImageResource::from_rgba(128, 128, rgba).unwrap(),
        );
        for index in 0..500 {
            art.document.layers.push(motion_art::Layer {
                id: format!("repeated{index}"),
                scene_id: script.scenes[1].id.clone(),
                name: "image".into(),
                visible: false,
                plane: motion_art::Plane::Foreground,
                active: None,
                opacity: 1.0,
                attachment: None,
                clip: None,
                clip_shape: None,
                blend: motion_art::BlendMode::Normal,
                content: motion_art::Content::Image {
                    asset_id: "pixel".into(),
                    rect: motion_art::Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 8.0,
                        height: 8.0,
                    },
                },
            });
        }
        let per_image = (art.images["pixel"].png().len() as u64).div_ceil(3) * 4 + 256;
        assert!(embedded_image_budget(&art, per_image * 499).is_err());
        assert_eq!(
            embedded_image_budget(&art, per_image * 500).unwrap(),
            per_image * 500
        );
        let dest = crate::tests::destination();
        for svg in [false, true] {
            let result = export_project_sequence(
                &script,
                &rendered,
                Some(&art),
                &dest,
                &CancellationToken::new(),
                |_| {},
                svg,
            );
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("embedded image byte budget"));
            assert!(!dest.exists());
        }
    }

    #[test]
    fn svg_uses_the_same_prepared_timed_mouths() {
        let (script, rendered) = crate::tests::fixture();
        let scenes = vec![rendered];
        let timeline = SequenceTimeline::compile(&script).unwrap();
        let cancel = CancellationToken::new();
        for sample in [0, 6000, 12000, 18000, 23999] {
            let (svg, _) =
                compose_svg(&script, &scenes, None, &timeline, sample, &cancel, None).unwrap();
            let svg = String::from_utf8(svg).unwrap();
            let raster =
                resvg::tiny_skia::Pixmap::decode_png(&media::rasterize_scaled(&svg, None).unwrap())
                    .unwrap();
            let (png, _) = compose(
                &script, &scenes, None, &timeline, sample, None, &cancel, None,
            )
            .unwrap();
            assert_eq!(raster.data(), png.data());
        }
    }

    #[test]
    fn svg_cancel_and_invalid_future_scene_preserve_destination() {
        let (script, mut rendered) = fixture(true);
        let dest = crate::tests::destination();
        let cancel = CancellationToken::new();
        cancel.cancel();
        assert!(matches!(
            export_svg_sequence(&script, &rendered, None, &dest, &cancel, |_| {}),
            Err(Error::Cancelled)
        ));
        assert!(!dest.exists());
        let cancel = CancellationToken::new();
        assert!(matches!(
            export_svg_sequence(&script, &rendered, None, &dest, &cancel, |_| cancel
                .cancel()),
            Err(Error::Cancelled)
        ));
        assert!(!dest.exists());
        rendered[1].manifest.scene_id = "invalid".into();
        assert!(export_svg_sequence(
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
        fs::create_dir(&dest).unwrap();
        fs::write(dest.join("keep"), b"keep").unwrap();
        assert!(export_svg_sequence(
            &script,
            &rendered,
            None,
            &dest,
            &CancellationToken::new(),
            |_| {}
        )
        .is_err());
        assert_eq!(fs::read(dest.join("keep")).unwrap(), b"keep");
        fs::remove_dir_all(dest).unwrap();
    }

    #[test]
    fn generated_svg_namespaces_every_definition_and_reference() {
        let source = "<svg><defs><clipPath id=\"canvas\"/><mask id=\"future\"/></defs><g clip-path=\"url(#canvas)\" mask=\"url(#future)\"><use href=\"#future\"/></g><text>url(#canvas) href=&quot;#future id=&quot;canvas</text></svg>";
        let namespaced = namespace_svg(source, "scene1_");
        assert!(namespaced.contains("id=\"scene1_future\""));
        assert!(namespaced.contains("url(#scene1_future)"));
        assert!(namespaced.contains("href=\"#scene1_future\""));
        assert!(namespaced.contains("<text>url(#canvas) href=&quot;#future id=&quot;canvas</text>"));
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
        let rendered = Arc::new(vec![motion_voice::export::render_scene(
            &script.scenes[0].id,
            30.0,
            &[],
        )
        .unwrap()]);
        let cancel = CancellationToken::new();
        let dest = crate::tests::destination();
        assert!(export_png_sequence(&script, &rendered, None, &dest, &cancel, |_| {}).is_err());
        assert!(!dest.exists());
        let preview = SequencePreview::new(
            Arc::new(script.clone()),
            Arc::clone(&rendered),
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
            clip_shape: None,
            blend: motion_art::BlendMode::Normal,
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
    #[test]
    fn advanced_clips_and_backdrop_blends_are_real_raster_pixels() {
        use motion_art::{BlendMode, ClipShape, Color, Content, Layer, Plane, Point, Rect};
        let (mut script, rendered) = fixture(false);
        script.scenes[0].composition.background_color = "#808080".into();
        let rect = Rect {
            x: 0.0,
            y: 0.0,
            width: 8.0,
            height: 8.0,
        };
        let mut art = ArtContext::default();
        art.document.layers.push(Layer {
            id: "blend".into(),
            scene_id: script.scenes[0].id.clone(),
            name: "blend".into(),
            visible: true,
            plane: Plane::Foreground,
            active: None,
            opacity: 1.0,
            attachment: None,
            clip: None,
            clip_shape: None,
            blend: BlendMode::Normal,
            content: Content::Rectangle {
                rect,
                color: Color {
                    r: 128,
                    g: 192,
                    b: 64,
                    a: 255,
                },
            },
        });
        let cancel = CancellationToken::new();
        for (mode, opacity, expected) in [
            (BlendMode::Normal, 1.0, [128, 192, 64]),
            (BlendMode::Multiply, 1.0, [64, 96, 32]),
            (BlendMode::Screen, 1.0, [192, 224, 160]),
            (BlendMode::Multiply, 0.5, [96, 112, 80]),
            (BlendMode::Screen, 0.5, [160, 176, 144]),
        ] {
            art.document.layers[0].blend = mode;
            art.document.layers[0].opacity = opacity;
            let frame =
                render_sequence_frame(&script, &rendered, Some(&art), 0, 8, &cancel).unwrap();
            for (actual, expected) in frame.rgba[..3].iter().zip(expected) {
                assert!(
                    (i32::from(*actual) - expected).abs() <= 2,
                    "{mode:?} opacity {opacity}: {:?}",
                    &frame.rgba[..4]
                );
            }
        }
        art.document.layers[0].blend = BlendMode::Normal;
        art.document.layers[0].opacity = 1.0;
        for shape in [
            ClipShape::Ellipse { rect },
            ClipShape::Polygon {
                points: vec![
                    Point { x: 4.0, y: 0.0 },
                    Point { x: 8.0, y: 4.0 },
                    Point { x: 4.0, y: 8.0 },
                    Point { x: 0.0, y: 4.0 },
                ],
            },
        ] {
            art.document.layers[0].clip_shape = Some(shape);
            let frame =
                render_sequence_frame(&script, &rendered, Some(&art), 0, 8, &cancel).unwrap();
            assert_eq!(&frame.rgba[..4], &[128, 128, 128, 255]);
            let center = (4 * 8 + 4) * 4;
            assert_eq!(&frame.rgba[center..center + 4], &[128, 192, 64, 255]);
            let again =
                render_sequence_frame(&script, &rendered, Some(&art), 0, 8, &cancel).unwrap();
            assert_eq!(frame.rgba, again.rgba);
            let dest = crate::tests::destination();
            export_png_sequence(&script, &rendered, Some(&art), &dest, &cancel, |_| {}).unwrap();
            let exported = resvg::tiny_skia::Pixmap::decode_png(
                &fs::read(dest.join("frame-000000.png")).unwrap(),
            )
            .unwrap();
            assert_eq!(frame.rgba, exported.data());
            fs::remove_dir_all(dest).unwrap();
        }
    }
    #[test]
    fn polygon_vertex_work_is_bounded_before_single_or_sequence_output() {
        use motion_art::{BlendMode, ClipShape, Color, Content, Layer, Plane, Point, Rect};
        let (mut script, rendered) = fixture(false);
        script.project.fps = 240;
        let points = (0..128)
            .map(|i| {
                let a = f64::from(i) * std::f64::consts::TAU / 128.0;
                Point {
                    x: 4.0 + 3.0 * a.cos(),
                    y: 4.0 + 3.0 * a.sin(),
                }
            })
            .collect();
        let base = Layer {
            id: "mask".into(),
            scene_id: script.scenes[0].id.clone(),
            name: "Mask".into(),
            visible: true,
            plane: Plane::Foreground,
            active: None,
            opacity: 1.0,
            attachment: None,
            clip: None,
            clip_shape: Some(ClipShape::Polygon { points }),
            blend: BlendMode::Normal,
            content: Content::Rectangle {
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 8.0,
                    height: 8.0,
                },
                color: Color {
                    r: 1,
                    g: 2,
                    b: 3,
                    a: 255,
                },
            },
        };
        let mut art = ArtContext::default();
        for i in 0..150 {
            let mut layer = base.clone();
            layer.id = format!("mask{i}");
            art.document.layers.push(layer);
        }
        art.document.validate().unwrap();
        let cancel = CancellationToken::new();
        let dest = crate::tests::destination();
        let single = crate::export_png_scene_with_art(
            &script,
            0,
            &rendered[0],
            &art,
            &dest,
            &cancel,
            |_| {},
        )
        .unwrap_err();
        assert!(single.to_string().contains("four million"), "{single}");
        assert!(!dest.exists());
        let sequence = export_png_sequence(&script, &rendered, Some(&art), &dest, &cancel, |_| {})
            .unwrap_err();
        assert!(sequence.to_string().contains("frame-work"), "{sequence}");
        assert!(!dest.exists());
    }
}
