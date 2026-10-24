//! Deterministic, self-contained SVG frames from evaluated scenes.
//! Uses original procedural characters; no fonts, network assets, or video encoder.
pub mod captions;
#[cfg(feature = "gpu")]
pub mod gpu;
#[cfg(feature = "gpu")]
pub mod gpu_scene;
#[cfg(feature = "gpu")]
pub mod scene_commands;

use motion_lipsync::Viseme;
use motion_scene::{
    EvaluatedScene, CHEST, HEAD, HIP, LEFT_FOOT, LEFT_HAND, RIGHT_FOOT, RIGHT_HAND,
};
use std::fmt::Write;

pub use captions::{
    append_captions, caption_font_data, validate_caption, CaptionCue, CAPTION_FONT_FAMILY,
};

pub const MAX_DIMENSION: u32 = 8192;
pub const MAX_ACTORS: usize = 256;
const MAX_LABEL_BYTES: usize = 1024;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("invalid artwork: {0}")]
    InvalidArtwork(String),
    #[error("scene and output dimensions must be between 1 and 8192 pixels")]
    InvalidDimensions,
    #[error("invalid evaluated camera, background color or scene opacity")]
    InvalidComposition,
    #[error("scene exceeds the 256 actor limit")]
    TooManyActors,
    #[error("actor {0} has invalid coordinates, wave envelope, or label")]
    InvalidActor(usize),
    #[error("custom rig texture unavailable: {0}")]
    RigTexture(String),
    #[error("caption: {0}")]
    Caption(String),
}

pub fn render_svg(scene: &EvaluatedScene, width: u32, height: u32) -> Result<String, Error> {
    render_svg_with_mouths(scene, width, height, &[])
}

/// Preserve scene aspect ratio with centered letterboxing. Actor order is scene
/// order, matching preview painter order. Unknown mouth IDs are ignored.
pub fn render_svg_with_mouths(
    scene: &EvaluatedScene,
    width: u32,
    height: u32,
    mouths: &[(&str, Viseme)],
) -> Result<String, Error> {
    render_scene(scene, width, height, mouths, None)
}

type Artwork<'a> = (
    &'a str,
    &'a motion_art::Document,
    &'a std::collections::BTreeMap<String, motion_art::ImageResource>,
);

fn bad_texture(part: &motion_rig::EvaluatedPart) -> bool {
    part.texture.as_ref().is_some_and(|t| {
        let m = t.transform;
        ![m.a, m.b, m.c, m.d, m.tx, m.ty]
            .iter()
            .chain(&t.uv)
            .chain(&t.rest_bounds)
            .all(|v| v.is_finite() && v.abs() <= 1_000_000.0)
            || t.rest_bounds[2] <= 0.0
            || t.rest_bounds[3] <= 0.0
            || t.uv[2] <= t.uv[0]
            || t.uv[3] <= t.uv[1]
    })
}

/// Append one textured part: the image sub-rectangle `uv` stretched over the
/// part's rest bounding box, carried by the evaluated transform and clipped to
/// the (possibly deformed) polygon. Embeds the decoded PNG as a data URI like
/// artwork images and charges it to the shared per-frame image byte budget.
fn append_rig_texture(
    svg: &mut String,
    part: &motion_rig::EvaluatedPart,
    clip_id: &str,
    images: Option<&std::collections::BTreeMap<String, motion_art::ImageResource>>,
    embedded: &mut u64,
) -> Result<(), Error> {
    let Some(texture) = &part.texture else {
        return Ok(());
    };
    let image = images
        .and_then(|images| images.get(&texture.asset))
        .ok_or_else(|| Error::RigTexture(format!("missing image resource {}", texture.asset)))?;
    *embedded += image.png().len() as u64;
    if *embedded > motion_art::MAX_IMAGE_BYTES {
        return Err(Error::RigTexture(
            "embedded rig texture bytes exceed the image budget".into(),
        ));
    }
    let [bx, by, bw, bh] = texture.rest_bounds;
    let [u0, v0, u1, v1] = texture.uv;
    let (w, h) = (bw / (u1 - u0), bh / (v1 - v0));
    let m = texture.transform;
    write!(svg, "<defs><clipPath id=\"{clip_id}\"><polygon points=\"").unwrap();
    for point in &part.points {
        write!(svg, "{:.6},{:.6} ", point.x, point.y).unwrap();
    }
    write!(
        svg,
        "\"/></clipPath></defs><g clip-path=\"url(#{clip_id})\"><image transform=\"matrix({:.6} {:.6} {:.6} {:.6} {:.6} {:.6})\" x=\"{:.6}\" y=\"{:.6}\" width=\"{w:.6}\" height=\"{h:.6}\" preserveAspectRatio=\"none\" href=\"{}\"/></g>",
        m.a, m.b, m.c, m.d, m.tx, m.ty, bx - u0 * w, by - v0 * h, image.data_uri()
    )
    .unwrap();
    Ok(())
}

/// One closed outline stroke with round joins (no miter spikes on sharp or
/// finely rounded corners). `stroke_width` is the full SVG stroke width; modes
/// drawn beneath a fill pass twice the visible band.
fn append_outline(
    svg: &mut String,
    part: &motion_rig::EvaluatedPart,
    color: [u8; 4],
    stroke_width: f64,
) {
    svg.push_str("<polygon points=\"");
    for point in &part.points {
        write!(svg, "{:.6},{:.6} ", point.x, point.y).unwrap();
    }
    write!(
        svg,
        "\" fill=\"none\" stroke=\"#{:02x}{:02x}{:02x}\" stroke-opacity=\"{:.6}\" stroke-width=\"{stroke_width:.6}\" stroke-linejoin=\"round\"/>",
        color[0],
        color[1],
        color[2],
        f64::from(color[3]) / 255.0
    )
    .unwrap();
}

pub(crate) fn validate_scene(scene: &EvaluatedScene, width: u32, height: u32) -> Result<(), Error> {
    if [scene.width, scene.height, width, height]
        .iter()
        .any(|&v| v == 0 || v > MAX_DIMENSION)
    {
        return Err(Error::InvalidDimensions);
    }
    let color = scene.background_color.as_bytes();
    if color.len() != 7
        || color[0] != b'#'
        || !color[1..].iter().all(u8::is_ascii_hexdigit)
        || !scene.time.is_finite()
        || scene.time < 0.0
        || !scene.opacity.is_finite()
        || !(0.0..=1.0).contains(&scene.opacity)
        || !scene.camera.x.is_finite()
        || !scene.camera.y.is_finite()
        || scene.camera.x.abs() > 1_000_000.0
        || scene.camera.y.abs() > 1_000_000.0
        || !scene.camera.zoom.is_finite()
        || !(0.05..=20.0).contains(&scene.camera.zoom)
    {
        return Err(Error::InvalidComposition);
    }
    if scene.actors.len() > MAX_ACTORS {
        return Err(Error::TooManyActors);
    }
    for (index, actor) in scene.actors.iter().enumerate() {
        if !actor.x.is_finite()
            || !actor.y.is_finite()
            || actor.x.abs() > 1_000_000.0
            || actor.y.abs() > 1_000_000.0
            || actor.skeleton.iter().any(|p| {
                !p.x.is_finite()
                    || !p.y.is_finite()
                    || p.x.abs() > 1_000_000.0
                    || p.y.abs() > 1_000_000.0
            })
            || actor.rigid_parts.len() > 256
            || actor.rigid_parts.iter().any(|part| {
                !(3..=motion_rig::MAX_EVALUATED_POINTS).contains(&part.points.len())
                    || bad_texture(part)
                    || part.outline.is_some_and(|o| {
                        !o.width.is_finite()
                            || !(0.0..=motion_rig::MAX_OUTLINE_WIDTH).contains(&o.width)
                    })
                    || part.points.iter().any(|p| {
                        !p.x.is_finite()
                            || !p.y.is_finite()
                            || p.x.abs() > 1_000_000.0
                            || p.y.abs() > 1_000_000.0
                    })
            })
            || !actor.rotation.is_finite()
            || actor.rotation.abs() > 360_000.0
            || !actor.scale.x.is_finite()
            || !actor.scale.y.is_finite()
            || !(0.001..=100.0).contains(&actor.scale.x)
            || !(0.001..=100.0).contains(&actor.scale.y)
            || !actor.wave.is_finite()
            || !(0.0..=1.0).contains(&actor.wave)
            || [&actor.id, &actor.emotion].iter().any(|s| {
                s.len() > MAX_LABEL_BYTES
                    || s.chars().any(|c| {
                        !matches!(c, '\t' | '\n' | '\r')
                            && (c < '\u{20}' || matches!(c, '\u{fffe}' | '\u{ffff}'))
                    })
            })
        {
            return Err(Error::InvalidActor(index));
        }
    }
    Ok(())
}

fn render_scene(
    scene: &EvaluatedScene,
    width: u32,
    height: u32,
    mouths: &[(&str, Viseme)],
    artwork: Option<Artwork<'_>>,
) -> Result<String, Error> {
    validate_scene(scene, width, height)?;
    let scale = (width as f64 / scene.width as f64).min(height as f64 / scene.height as f64);
    let ox = (width as f64 - scene.width as f64 * scale) * 0.5;
    let oy = (height as f64 - scene.height as f64 * scale) * 0.5;
    let (camera_x, camera_y) = scene.camera.project(0.0, 0.0, scene.width, scene.height);
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><defs><clipPath id=\"canvas\"><rect width=\"{}\" height=\"{}\"/></clipPath></defs><rect width=\"100%\" height=\"100%\" fill=\"black\"/><g transform=\"translate({ox:.6} {oy:.6}) scale({scale:.6})\" clip-path=\"url(#canvas)\" opacity=\"{:.6}\"><rect width=\"{}\" height=\"{}\" fill=\"{}\"/><g transform=\"translate({camera_x:.6} {camera_y:.6}) scale({:.6})\">",
        scene.width, scene.height, scene.opacity, scene.width, scene.height, scene.background_color, scene.camera.zoom
    );
    if let Some((scene_id, document, images)) = artwork {
        append_art(
            &mut svg,
            scene,
            scene_id,
            document,
            images,
            motion_art::Plane::Background,
        )?;
    }
    if let Some((scene_id, document, images)) = artwork {
        if document.lights.iter().any(|light| {
            light.scene_id == scene_id && light.behind_actors && light.is_active(scene.time)
        }) {
            // Close the camera while drawing canvas-space tints, then reopen it
            // for actors. Preserve legacy SVG bytes when no backlight is active.
            svg.push_str("</g>");
            let camera = format!(
                "translate({camera_x:.6} {camera_y:.6}) scale({:.6})",
                scene.camera.zoom
            );
            append_lights(&mut svg, scene, scene_id, document, images, &camera, true)?;
            write!(svg, "<g transform=\"{camera}\">").unwrap();
        }
    }
    let mut embedded_rig_bytes = 0u64;
    let rig_images = artwork.map(|(_, _, images)| images);
    for (actor_index, actor) in scene.actors.iter().enumerate() {
        write!(
            svg,
            "<g transform=\"translate({:.6} {:.6})\">",
            actor.x, actor.y
        )
        .unwrap();
        if actor.rotation != 0.0 || actor.scale.x != 1.0 || actor.scale.y != 1.0 {
            write!(
                svg,
                "<g transform=\"rotate({:.6}) scale({:.6} {:.6})\">",
                actor.rotation.to_degrees(),
                actor.scale.x,
                actor.scale.y
            )
            .unwrap();
        }
        if !actor.rigid_parts.is_empty() {
            use motion_rig::OutlineMode;
            // Silhouette outlines first, beneath every fill of this actor.
            for part in &actor.rigid_parts {
                if let Some(o) = part.outline.filter(|o| o.mode == OutlineMode::Silhouette) {
                    append_outline(&mut svg, part, o.color, o.width * 2.0);
                }
            }
            for (part_index, part) in actor.rigid_parts.iter().enumerate() {
                if let Some(o) = part.outline.filter(|o| o.mode == OutlineMode::Behind) {
                    append_outline(&mut svg, part, o.color, o.width * 2.0);
                }
                svg.push_str("<polygon points=\"");
                for point in &part.points {
                    write!(svg, "{:.6},{:.6} ", point.x, point.y).unwrap();
                }
                write!(
                    svg,
                    "\" fill=\"#{:02x}{:02x}{:02x}\" fill-opacity=\"{:.6}\"/>",
                    part.color[0],
                    part.color[1],
                    part.color[2],
                    f64::from(part.color[3]) / 255.0
                )
                .unwrap();
                append_rig_texture(
                    &mut svg,
                    part,
                    &format!("rigtex{actor_index}x{part_index}"),
                    rig_images,
                    &mut embedded_rig_bytes,
                )?;
                if let Some(o) = part.outline.filter(|o| o.mode == OutlineMode::Over) {
                    append_outline(&mut svg, part, o.color, o.width);
                }
            }
        } else {
            let p = &actor.skeleton;
            svg.push_str("<path d=\"");
            for (a, b) in [
                (HIP, CHEST),
                (CHEST, HEAD),
                (CHEST, LEFT_HAND),
                (CHEST, RIGHT_HAND),
                (HIP, LEFT_FOOT),
                (HIP, RIGHT_FOOT),
            ] {
                write!(
                    svg,
                    "M{:.6} {:.6}L{:.6} {:.6}",
                    p[a].x, p[a].y, p[b].x, p[b].y
                )
                .unwrap();
            }
            svg.push_str(
                "\" fill=\"none\" stroke=\"#7eb9e2\" stroke-width=\"5\" stroke-linecap=\"round\"/>",
            );
            write!(svg,"<g transform=\"translate({:.6} {:.6})\"><circle r=\"25\" fill=\"#f5c8a2\"/><g fill=\"black\"><circle cx=\"-8\" cy=\"-4\" r=\"2.5\"/><circle cx=\"8\" cy=\"-4\" r=\"2.5\"/></g>",p[HEAD].x,p[HEAD].y).unwrap();
            let viseme = mouths
                .iter()
                .find(|(id, _)| *id == actor.id)
                .map(|(_, v)| *v)
                .unwrap_or(Viseme::Rest);
            let (w, h) = match viseme {
                Viseme::A => (14.0, 14.0),
                Viseme::O | Viseme::U => (8.0, 11.0),
                Viseme::E | Viseme::I => (18.0, 6.0),
                Viseme::Rest | Viseme::Closed => (13.0, 2.0),
                _ => (12.0, 5.0),
            };
            write!(
                svg,
                "<rect x=\"{}\" y=\"{}\" width=\"{w}\" height=\"{h}\" rx=\"2\" fill=\"#7d2d39\"/>",
                -w / 2.0,
                10.0 - h / 2.0
            )
            .unwrap();
            if actor.emotion == "happy" && matches!(viseme, Viseme::Rest | Viseme::Closed) {
                svg.push_str(
                "<path d=\"M-8 9L0 14L8 9\" fill=\"none\" stroke=\"#7d2d39\" stroke-width=\"2\"/>",
            );
            }
            if actor.emotion == "surprised" {
                svg.push_str("<path d=\"M-12 -13L-4 -15M4 -13L12 -15\" stroke=\"black\"/>");
            }
            svg.push_str("</g>");
        }
        if actor.rotation != 0.0 || actor.scale.x != 1.0 || actor.scale.y != 1.0 {
            svg.push_str("</g>");
        }
        write!(svg, "<text display=\"none\" y=\"45\" text-anchor=\"middle\" dominant-baseline=\"middle\" font-family=\"sans-serif\" font-size=\"12\" fill=\"white\">{} · {}</text></g>", escape(&actor.id), escape(&actor.emotion)).unwrap();
    }
    if let Some((scene_id, document, images)) = artwork {
        append_art(
            &mut svg,
            scene,
            scene_id,
            document,
            images,
            motion_art::Plane::Foreground,
        )?;
    }
    svg.push_str("</g>");
    if let Some((scene_id, document, images)) = artwork {
        let camera = format!(
            "translate({camera_x:.6} {camera_y:.6}) scale({:.6})",
            scene.camera.zoom
        );
        append_lights(&mut svg, scene, scene_id, document, images, &camera, false)?;
    }
    svg.push_str("</g></svg>\n");
    Ok(svg)
}

/// Timed artwork in document order within each plane, sharing camera, clip and fade.
/// Attachments intentionally inherit actor translation only, matching native artwork.
#[allow(clippy::too_many_arguments)]
pub fn render_svg_with_art(
    scene: &EvaluatedScene,
    width: u32,
    height: u32,
    mouths: &[(&str, Viseme)],
    scene_id: &str,
    document: &motion_art::Document,
    images: &std::collections::BTreeMap<String, motion_art::ImageResource>,
) -> Result<String, Error> {
    document
        .validate()
        .map_err(|e| Error::InvalidArtwork(e.to_string()))?;
    render_scene(
        scene,
        width,
        height,
        mouths,
        Some((scene_id, document, images)),
    )
}

/// Content extent including round stroke caps. Input has already been validated.
fn artwork_content_bounds(content: &motion_art::Content) -> motion_art::Rect {
    match content {
        motion_art::Content::Image { rect, .. }
        | motion_art::Content::AnimatedImage { rect, .. }
        | motion_art::Content::Rectangle { rect, .. }
        | motion_art::Content::Ellipse { rect, .. } => *rect,
        motion_art::Content::Stroke { points, width, .. } => {
            let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for p in points {
                x0 = x0.min(p.x);
                y0 = y0.min(p.y);
                x1 = x1.max(p.x);
                y1 = y1.max(p.y);
            }
            motion_art::Rect {
                x: x0 - width / 2.0,
                y: y0 - width / 2.0,
                width: x1 - x0 + width,
                height: y1 - y0 + width,
            }
        }
    }
}

fn append_art(
    svg: &mut String,
    scene: &EvaluatedScene,
    scene_id: &str,
    document: &motion_art::Document,
    images: &std::collections::BTreeMap<String, motion_art::ImageResource>,
    plane: motion_art::Plane,
) -> Result<(), Error> {
    use motion_art::Content;
    let steps = document
        .composition(scene_id, plane, scene.time)
        .map_err(|e| Error::InvalidArtwork(e.to_string()))?;
    for step in steps {
        let layer = match step {
            motion_art::CompositionStep::EnterGroup(group) => {
                let blend = match group.blend {
                    motion_art::BlendMode::Normal => "normal",
                    motion_art::BlendMode::Multiply => "multiply",
                    motion_art::BlendMode::Screen => "screen",
                };
                write!(
                    svg,
                    "<g opacity=\"{:.6}\" style=\"isolation:isolate;mix-blend-mode:{blend}\">",
                    group.opacity
                )
                .unwrap();
                continue;
            }
            motion_art::CompositionStep::LeaveGroup => {
                svg.push_str("</g>");
                continue;
            }
            motion_art::CompositionStep::Layer(layer) => layer,
        };
        let index = document
            .layers
            .iter()
            .position(|l| l.id == layer.id)
            .unwrap();
        let (x, y) = match &layer.attachment {
            Some(id) => match scene.actors.iter().find(|a| &a.id == id) {
                Some(a) => (a.x, a.y),
                None => continue,
            },
            None => (0.0, 0.0),
        };
        let blend_style = match layer.blend {
            motion_art::BlendMode::Normal => "",
            motion_art::BlendMode::Multiply => " style=\"mix-blend-mode:multiply\"",
            motion_art::BlendMode::Screen => " style=\"mix-blend-mode:screen\"",
        };
        write!(
            svg,
            "<g transform=\"translate({x:.6} {y:.6})\" opacity=\"{:.6}\"{blend_style}>",
            layer.opacity
        )
        .unwrap();
        if let Some(r) = layer.clip {
            write!(svg,"<defs><clipPath id=\"art{index}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath></defs><g clip-path=\"url(#art{index})\">",r.x,r.y,r.width,r.height).unwrap();
        }
        if let Some(motion_art::ClipShape::Painted {
            base_visible,
            strokes,
            ..
        }) = &layer.clip_shape
        {
            // Bound the mask to content in layer-local coordinates, rather than
            // the viewport: camera moves and actor attachment must not crop it.
            let r = artwork_content_bounds(&layer.content);
            let base = if *base_visible { "white" } else { "black" };
            write!(svg, "<defs><mask id=\"art{index}\" mask-type=\"luminance\" maskUnits=\"userSpaceOnUse\" maskContentUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{base}\"/>", r.x, r.y, r.width, r.height, r.x, r.y, r.width, r.height).unwrap();
            let motion = layer
                .clip_shape
                .as_ref()
                .and_then(|shape| shape.sample_motion(scene.time));
            if let Some(motion) = &motion {
                write!(svg, "<g transform=\"{}\">", motion.svg_transform()).unwrap();
            }
            for (stroke_index, stroke) in strokes.iter().enumerate() {
                let color = if stroke.erase { "black" } else { "white" };
                if !stroke.pressures.is_empty() || stroke.hardness != 1.0 {
                    write!(svg, "<defs><mask id=\"art{index}stroke{stroke_index}\" mask-type=\"luminance\" maskUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"black\"/>", r.x, r.y, r.width, r.height, r.x, r.y, r.width, r.height).unwrap();
                    for (coverage, scale) in stroke.coverage_bands() {
                        write!(svg, "<path fill=\"rgb({0}%,{0}%,{0}%)\" d=\"", coverage * 100.0).unwrap();
                        for primitive in stroke.band_geometry(scale) {
                            match primitive {
                                motion_art::MaskBrushPrimitive::Circle { center, radius } => {
                                    write!(svg, "M{} {} a{radius} {radius} 0 1 1 {} 0 a{radius} {radius} 0 1 1 {} 0 Z ", center.x + radius, center.y, -2.0 * radius, 2.0 * radius).unwrap();
                                }
                                motion_art::MaskBrushPrimitive::Polygon { points } => {
                                    for (i, p) in points.iter().enumerate() {
                                        let command = if i == 0 { 'M' } else { 'L' };
                                        write!(svg, "{command}{} {} ", p.x, p.y).unwrap();
                                    }
                                    svg.push_str("Z ");
                                }
                            }
                        }
                        svg.push_str("\"/>");
                    }
                    write!(svg, "</mask></defs><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"{color}\" mask=\"url(#art{index}stroke{stroke_index})\"/>", r.x, r.y, r.width, r.height).unwrap();
                } else if stroke.points.len() == 1 {
                    let p = stroke.points[0];
                    write!(svg, "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"{color}\"/>", p.x, p.y, stroke.radius).unwrap();
                } else {
                    svg.push_str("<path d=\"");
                    for (i, p) in stroke.points.iter().enumerate() {
                        let command = if i == 0 { 'M' } else { 'L' };
                        write!(svg, "{command}{} {} ", p.x, p.y).unwrap();
                    }
                    write!(svg, "\" fill=\"none\" stroke=\"{color}\" stroke-width=\"{}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>", stroke.radius * 2.0).unwrap();
                }
            }
            if motion.is_some() {
                svg.push_str("</g>");
            }
            write!(svg, "</mask></defs><g mask=\"url(#art{index})\">").unwrap();
        } else if let Some(motion_art::ClipShape::SoftEllipse { .. }) = &layer.clip_shape {
            let (r, feather) = layer
                .clip_shape
                .as_ref()
                .unwrap()
                .sample_ellipse(scene.time)
                .unwrap();
            if feather == 0.0 {
                write!(svg, "<defs><clipPath id=\"art{index}\"><ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"/></clipPath></defs><g clip-path=\"url(#art{index})\">", r.x + r.width / 2.0, r.y + r.height / 2.0, r.width / 2.0, r.height / 2.0).unwrap();
            } else {
                write!(svg, "<defs><radialGradient id=\"art{index}fade\"><stop offset=\"{}\" stop-color=\"white\"/><stop offset=\"1\" stop-color=\"white\" stop-opacity=\"0\"/></radialGradient><mask id=\"art{index}\" maskUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" fill=\"url(#art{index}fade)\"/></mask></defs><g mask=\"url(#art{index})\">", 1.0 - feather, r.x, r.y, r.width, r.height, r.x, r.y, r.width, r.height).unwrap();
            }
        } else if let Some(motion_art::ClipShape::Polygon {
            points, feather, ..
        }) = layer
            .clip_shape
            .as_ref()
            .filter(|shape| matches!(shape, motion_art::ClipShape::Polygon { feather, .. } if *feather > 0.0))
        {
            // Blurred white polygon as a luminance-free alpha mask; the region
            // covers four standard deviations so the tails are not truncated.
            let motion = layer
                .clip_shape
                .as_ref()
                .and_then(|shape| shape.sample_motion(scene.time));
            let margin = 4.0 * feather;
            let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
            for p in points {
                x0 = x0.min(p.x);
                y0 = y0.min(p.y);
                x1 = x1.max(p.x);
                y1 = y1.max(p.y);
            }
            let (x, y, w, h) = (x0 - margin, y0 - margin, x1 - x0 + 2.0 * margin, y1 - y0 + 2.0 * margin);
            // A moving polygon may leave its static bounds; the content bounds
            // are the largest region the mask can ever need to cover.
            let region = if motion.is_some() {
                let r = artwork_content_bounds(&layer.content);
                (r.x, r.y, r.width, r.height)
            } else {
                (x, y, w, h)
            };
            write!(svg, "<defs><filter id=\"art{index}blur\" filterUnits=\"userSpaceOnUse\" x=\"{x}\" y=\"{y}\" width=\"{w}\" height=\"{h}\"><feGaussianBlur stdDeviation=\"{feather}\"/></filter><mask id=\"art{index}\" maskUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\">", region.0, region.1, region.2, region.3).unwrap();
            if let Some(motion) = &motion {
                write!(svg, "<g transform=\"{}\">", motion.svg_transform()).unwrap();
            }
            svg.push_str("<polygon fill=\"white\" filter=\"url(#art");
            write!(svg, "{index}blur)\" points=\"").unwrap();
            for p in points {
                write!(svg, "{},{} ", p.x, p.y).unwrap();
            }
            svg.push_str("\"/>");
            if motion.is_some() {
                svg.push_str("</g>");
            }
            write!(svg, "</mask></defs><g mask=\"url(#art{index})\">").unwrap();
        } else if let Some(shape @ motion_art::ClipShape::Image {
            asset_id,
            channel,
            ..
        }) = &layer.clip_shape
        {
            let r = shape.sample_image_rect(scene.time).ok_or_else(|| {
                Error::InvalidArtwork("image mask rectangle is unavailable".into())
            })?;
            let image = images.get(asset_id).ok_or_else(|| {
                Error::InvalidArtwork(format!("missing decoded mask image {asset_id}"))
            })?;
            if image.png().len() > motion_art::MAX_IMAGE_BYTES as usize {
                return Err(Error::InvalidArtwork(
                    "image encoded byte budget exceeded".into(),
                ));
            }
            let mask_type = match channel {
                motion_art::MaskChannel::Alpha => "alpha",
                motion_art::MaskChannel::Luminance => "luminance",
            };
            write!(svg, "<defs><mask id=\"art{index}\" mask-type=\"{mask_type}\" maskUnits=\"userSpaceOnUse\" x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"><image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"{}\"/></mask></defs><g mask=\"url(#art{index})\">", r.x, r.y, r.width, r.height, r.x, r.y, r.width, r.height, image.data_uri()).unwrap();
        } else if let Some(shape) = &layer.clip_shape {
            write!(svg, "<defs><clipPath id=\"art{index}\">").unwrap();
            match shape {
                motion_art::ClipShape::Ellipse { rect: r } => {
                    write!(
                        svg,
                        "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\"/>",
                        r.x + r.width / 2.0,
                        r.y + r.height / 2.0,
                        r.width / 2.0,
                        r.height / 2.0
                    )
                    .unwrap();
                }
                motion_art::ClipShape::SoftEllipse { .. }
                | motion_art::ClipShape::Image { .. }
                | motion_art::ClipShape::Painted { .. } => {
                    return Err(Error::InvalidArtwork("invalid soft-mask dispatch".into()))
                }
                motion_art::ClipShape::Polygon { points, .. } => {
                    svg.push_str("<polygon");
                    if let Some(motion) = shape.sample_motion(scene.time) {
                        write!(svg, " transform=\"{}\"", motion.svg_transform()).unwrap();
                    }
                    svg.push_str(" points=\"");
                    for p in points {
                        write!(svg, "{},{} ", p.x, p.y).unwrap();
                    }
                    svg.push_str("\"/>");
                }
            }
            write!(svg, "</clipPath></defs><g clip-path=\"url(#art{index})\">").unwrap();
        }
        let paint = |color: &motion_art::Color| {
            format!(
                "fill=\"#{:02x}{:02x}{:02x}\" fill-opacity=\"{:.6}\"",
                color.r,
                color.g,
                color.b,
                f64::from(color.a) / 255.0
            )
        };
        match &layer.content {
            Content::Image { asset_id, rect: r } => {
                let image = images.get(asset_id).ok_or_else(|| {
                    Error::InvalidArtwork(format!("missing decoded image {asset_id}"))
                })?;
                if image.png().len() > motion_art::MAX_IMAGE_BYTES as usize {
                    return Err(Error::InvalidArtwork(
                        "image encoded byte budget exceeded".into(),
                    ));
                }
                write!(svg,"<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"{}\"/>",r.x,r.y,r.width,r.height,image.data_uri()).unwrap();
            }
            Content::AnimatedImage { rect: r, .. } => {
                // Exactly one frame is embedded: the one selected at scene time.
                let asset_id = layer.animated_asset(scene.time).ok_or_else(|| {
                    Error::InvalidArtwork("animated image has no selectable frame".into())
                })?;
                let image = images.get(asset_id).ok_or_else(|| {
                    Error::InvalidArtwork(format!("missing decoded image {asset_id}"))
                })?;
                if image.png().len() > motion_art::MAX_IMAGE_BYTES as usize {
                    return Err(Error::InvalidArtwork(
                        "image encoded byte budget exceeded".into(),
                    ));
                }
                write!(svg,"<image x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" preserveAspectRatio=\"none\" href=\"{}\"/>",r.x,r.y,r.width,r.height,image.data_uri()).unwrap();
            }
            Content::Rectangle { rect: r, color } => {
                write!(
                    svg,
                    "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" {}/>",
                    r.x,
                    r.y,
                    r.width,
                    r.height,
                    paint(color)
                )
                .unwrap();
            }
            Content::Ellipse { rect: r, color } => {
                write!(
                    svg,
                    "<ellipse cx=\"{}\" cy=\"{}\" rx=\"{}\" ry=\"{}\" {}/>",
                    r.x + r.width / 2.0,
                    r.y + r.height / 2.0,
                    r.width / 2.0,
                    r.height / 2.0,
                    paint(color)
                )
                .unwrap();
            }
            Content::Stroke {
                points,
                color,
                width,
            } => {
                svg.push_str("<polyline points=\"");
                for p in points {
                    write!(svg, "{},{} ", p.x, p.y).unwrap();
                }
                write!(svg,"\" fill=\"none\" stroke=\"#{:02x}{:02x}{:02x}\" stroke-opacity=\"{:.6}\" stroke-width=\"{width}\" stroke-linecap=\"round\" stroke-linejoin=\"round\"/>",color.r,color.g,color.b,f64::from(color.a)/255.0).unwrap();
            }
        }
        if layer.clip.is_some() || layer.clip_shape.is_some() {
            svg.push_str("</g>");
        }
        svg.push_str("</g>");
        if svg.len() > 48 * 1024 * 1024 {
            return Err(Error::InvalidArtwork(
                "SVG artwork exceeds 48 MiB frame budget".into(),
            ));
        }
    }
    Ok(())
}

fn hex(color: &motion_art::Color) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

pub(crate) struct SampledRadialLight { pub position: motion_art::Point, pub rgb: [f64;3], pub alpha: f64 }

pub(crate) fn sample_radial_light(scene: &EvaluatedScene, document: &motion_art::Document,
    images: &std::collections::BTreeMap<String,motion_art::ImageResource>, light: &motion_art::Light) -> Result<Option<SampledRadialLight>,Error> {
    let motion_art::LightSource::Radial {color,intensity,attachment,screen_glow,..} = &light.source else {return Ok(None);};
        let mut position = light.sample_position(scene.time).expect("radial light");
        let mut rgb = [
            f64::from(color.r) / 255.0,
            f64::from(color.g) / 255.0,
            f64::from(color.b) / 255.0,
        ];
        let mut alpha = intensity * light.flicker_factor(scene.time);
        if let Some(id) = attachment {
            match scene.actors.iter().find(|a| &a.id == id) {
                Some(a) => {
                    position.x += a.x;
                    position.y += a.y;
                }
                None => return Ok(None),
            }
        }
        if let Some(glow) = screen_glow {
            let Some(prop) = document
                .layers
                .iter()
                .find(|l| l.id == glow.layer_id && l.is_active(scene.time))
            else {
                return Ok(None);
            };
            let (asset_id, rect) = match &prop.content {
                motion_art::Content::Image { asset_id, rect } => (asset_id.as_str(), *rect),
                motion_art::Content::AnimatedImage { rect, .. } => {
                    (prop.animated_asset(scene.time).unwrap_or_default(), *rect)
                }
                _ => return Ok(None),
            };
            let image = images.get(asset_id).ok_or_else(|| {
                Error::InvalidArtwork(format!("missing decoded image {asset_id}"))
            })?;
            let avg = image.average_rgb();
            let peak = avg.iter().cloned().fold(0.0, f64::max);
            if peak <= 0.0 {
                return Ok(None);
            }
            let luma = 0.2126 * avg[0] + 0.7152 * avg[1] + 0.0722 * avg[2];
            for (channel, value) in rgb.iter_mut().enumerate() {
                *value += (avg[channel] / peak - *value) * glow.tint;
            }
            alpha = (alpha * glow.gain * luma).clamp(0.0, 1.0);
            position.x += rect.x + rect.width / 2.0;
            position.y += rect.y + rect.height / 2.0;
            if let Some(owner) = &prop.attachment {
                match scene.actors.iter().find(|a| &a.id == owner) {
                    Some(a) => {
                        position.x += a.x;
                        position.y += a.y;
                    }
                    None => return Ok(None),
                }
            }
        }
    Ok(Some(SampledRadialLight { position,rgb,alpha }))
}

/// Lights on the selected side of the actors. Ambient tint and vignette are canvas-space
/// (outside the camera); radial lights live in scene space under `camera`.
/// Ambient/vignette come first so glows stay bright over a darkened room.
#[allow(clippy::too_many_arguments)]
fn append_lights(
    svg: &mut String,
    scene: &EvaluatedScene,
    scene_id: &str,
    document: &motion_art::Document,
    images: &std::collections::BTreeMap<String, motion_art::ImageResource>,
    camera: &str,
    behind_actors: bool,
) -> Result<(), Error> {
    use motion_art::LightSource;
    let active: Vec<(usize, &motion_art::Light)> = document
        .lights
        .iter()
        .enumerate()
        .filter(|(_, l)| {
            l.scene_id == scene_id && l.behind_actors == behind_actors && l.is_active(scene.time)
        })
        .collect();
    if active.is_empty() {
        return Ok(());
    }
    for (index, light) in &active {
        let flicker = light.flicker_factor(scene.time);
        match &light.source {
            LightSource::Ambient { color, strength } => {
                write!(svg, "<rect width=\"{}\" height=\"{}\" fill=\"{}\" opacity=\"{:.6}\" style=\"mix-blend-mode:multiply\"/>", scene.width, scene.height, hex(color), strength * flicker).unwrap();
            }
            LightSource::Vignette { strength, inner } => {
                write!(svg, "<defs><radialGradient id=\"light{index}\" cx=\"0.5\" cy=\"0.5\" r=\"0.707107\"><stop offset=\"{inner:.6}\" stop-color=\"#000000\" stop-opacity=\"0\"/><stop offset=\"1\" stop-color=\"#000000\" stop-opacity=\"{:.6}\"/></radialGradient></defs><rect width=\"{}\" height=\"{}\" fill=\"url(#light{index})\"/>", strength * flicker, scene.width, scene.height).unwrap();
            }
            LightSource::Radial { .. } => {}
        }
    }
    let mut opened = false;
    for (index, light) in &active {
        let LightSource::Radial {
            radius,
            aspect,
            softness,
            blend,
            ..
        } = &light.source
        else {
            continue;
        };
        let Some(sample) = sample_radial_light(scene, document, images, light)? else {continue;};
        let SampledRadialLight { position, rgb, alpha } = sample;
        if !opened {
            write!(svg, "<g transform=\"{camera}\">").unwrap();
            opened = true;
        }
        let stop_color = format!(
            "#{:02x}{:02x}{:02x}",
            (rgb[0] * 255.0).round() as u8,
            (rgb[1] * 255.0).round() as u8,
            (rgb[2] * 255.0).round() as u8
        );
        write!(
            svg,
            "<defs><radialGradient id=\"light{index}\" cx=\"0.5\" cy=\"0.5\" r=\"0.5\">"
        )
        .unwrap();
        // Smoothstep falloff over the softness band; flat before it.
        let inner = 1.0 - softness;
        write!(
            svg,
            "<stop offset=\"0\" stop-color=\"{stop_color}\" stop-opacity=\"{alpha:.6}\"/>"
        )
        .unwrap();
        if *softness > 0.0 {
            for step in 0..=6 {
                let f = f64::from(step) / 6.0;
                let fade = 1.0 - f * f * (3.0 - 2.0 * f);
                write!(
                    svg,
                    "<stop offset=\"{:.6}\" stop-color=\"{stop_color}\" stop-opacity=\"{:.6}\"/>",
                    inner + softness * f,
                    alpha * fade
                )
                .unwrap();
            }
        } else {
            write!(
                svg,
                "<stop offset=\"1\" stop-color=\"{stop_color}\" stop-opacity=\"{alpha:.6}\"/>"
            )
            .unwrap();
        }
        let blend_style = match blend {
            motion_art::BlendMode::Normal => "",
            motion_art::BlendMode::Multiply => " style=\"mix-blend-mode:multiply\"",
            motion_art::BlendMode::Screen => " style=\"mix-blend-mode:screen\"",
        };
        write!(svg, "</radialGradient></defs><ellipse cx=\"{:.6}\" cy=\"{:.6}\" rx=\"{radius:.6}\" ry=\"{:.6}\" fill=\"url(#light{index})\"{blend_style}/>", position.x, position.y, radius * aspect).unwrap();
    }
    if opened {
        svg.push_str("</g>");
    }
    Ok(())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use motion_scene::ActorPose;
    pub(super) fn scene() -> EvaluatedScene {
        EvaluatedScene {
            time: 0.0,
            camera: motion_scene::CameraPose::default(),
            background_color: "#181f2c".into(),
            opacity: 1.0,
            width: 640,
            height: 360,
            actors: vec![ActorPose {
                id: "EMMA".into(),
                x: 120.0,
                y: 250.0,
                emotion: "happy".into(),
                rotation: 0.0,
                scale: motion_scene::Vec2::new(1.0, 1.0),
                wave: 1.0,
                skeleton: motion_animation::humanoid(None, 1.0),
                rigid_parts: Vec::new(),
                mouth: None,
            }],
        }
    }
    #[test]
    fn custom_rigid_parts_share_evaluated_hierarchy_and_actor_transform() {
        let source = "characters:\n  PUPPET:\n    asset: custom\n    rig:\n      version: 1\n      id: puppet\n      joints: [{id: root, parent: null, x: 10, y: -20}]\n      parts: [{id: body, joint: root, points: [[0,0],[10,0],[0,10]], color: [20,40,80,128]}]\nscenes: [{id: intro, duration: 2, events: []}]\n";
        let script = motion_script::parse(source).unwrap();
        let mut evaluated = motion_scene::evaluate(&script, 0, 0.5).unwrap();
        evaluated.actors[0].rotation = std::f64::consts::FRAC_PI_2;
        let svg = render_svg(&evaluated, 640, 360).unwrap();
        assert!(svg.contains(
            "<polygon points=\"10.000000,-20.000000 20.000000,-20.000000 10.000000,-10.000000 "
        ));
        assert!(svg.contains("fill=\"#142850\" fill-opacity=\"0.501961\""));
        assert!(svg.contains("rotate(90.000000)"));
        assert!(!svg.contains("<circle r=\"25\""));
        // Legacy rigs: no outline strokes and no new serialized fields.
        assert!(!svg.contains("stroke-linejoin"));
        let rig = serde_json::to_value(script.characters["PUPPET"].rig.as_ref().unwrap()).unwrap();
        let keys =
            |v: &serde_json::Value| v.as_object().unwrap().keys().cloned().collect::<Vec<_>>();
        assert_eq!(keys(&rig), ["id", "joints", "parts", "version"]);
        assert_eq!(keys(&rig["parts"][0]), ["color", "id", "joint", "points"]);
        evaluated.actors[0].rigid_parts[0].points[0].x = f64::NAN;
        assert_eq!(
            render_svg(&evaluated, 640, 360),
            Err(Error::InvalidActor(0))
        );
    }
    #[test]
    fn outline_modes_order_strokes_around_fills() {
        let source = "characters:\n  PUPPET:\n    asset: custom\n    rig:\n      version: 1\n      id: puppet\n      joints: [{id: root, parent: null}]\n      parts:\n        - {id: a, joint: root, points: [[0,0],[10,0],[0,10]], color: [1,1,1,255], outline: {width: 2, mode: behind}}\n        - {id: b, joint: root, ellipse: {rx: 5, ry: 4}, color: [2,2,2,255], outline: {width: 1, color: [9,8,7,128]}}\n        - {id: c, joint: root, points: [[0,0],[10,0],[10,10],[0,10]], smooth: 2, color: [3,3,3,255], outline: {width: 3, mode: silhouette}}\nscenes: [{id: intro, duration: 2, events: []}]\n";
        let script = motion_script::parse(source).unwrap();
        let evaluated = motion_scene::evaluate(&script, 0, 0.5).unwrap();
        let svg = render_svg(&evaluated, 640, 360).unwrap();
        let at = |needle: &str| svg.find(needle).unwrap_or_else(|| panic!("{needle}"));
        let silhouette = at("stroke-width=\"6.000000\"");
        let behind = at("stroke-width=\"4.000000\"");
        let fill_a = at("fill=\"#010101\"");
        let fill_b = at("fill=\"#020202\"");
        let over = at("stroke=\"#090807\" stroke-opacity=\"0.501961\" stroke-width=\"1.000000\"");
        let fill_c = at("fill=\"#030303\"");
        assert!(silhouette < behind && behind < fill_a && fill_a < fill_b);
        assert!(fill_b < over && over < fill_c);
        assert_eq!(svg.matches("stroke-linejoin=\"round\"").count(), 3);
        // Ellipse and rounded parts are dense polygons.
        let parts = &evaluated.actors[0].rigid_parts;
        assert_eq!((parts[1].points.len(), parts[2].points.len()), (48, 8));
        let mut bad = evaluated.clone();
        bad.actors[0].rigid_parts[0].outline.as_mut().unwrap().width = f64::INFINITY;
        assert_eq!(render_svg(&bad, 640, 360), Err(Error::InvalidActor(0)));
    }
    #[test]
    fn deterministic_geometry_and_letterbox() {
        let s = scene();
        let svg = render_svg(&s, 640, 480).unwrap();
        assert_eq!(svg, render_svg(&s, 640, 480).unwrap());
        assert!(svg.contains("translate(0.000000 60.000000) scale(1.000000)"));
        assert!(svg.contains("translate(120.000000 250.000000)"));
        assert!(svg.contains("L30.000000 -85.000000"));
        assert!(svg.contains("M-8 9L0 14L8 9"));
        let mut moved = s.clone();
        moved.actors[0].x = 200.0;
        assert_ne!(svg, render_svg(&moved, 640, 480).unwrap());
    }
    #[test]
    fn root_rotation_and_scale_enclose_body_and_face_geometry() {
        let mut s = scene();
        s.actors[0].rotation = std::f64::consts::FRAC_PI_2;
        s.actors[0].scale = motion_scene::Vec2::new(2.0, 0.5);
        let svg = render_svg(&s, 640, 360).unwrap();
        assert!(svg.contains("rotate(90.000000) scale(2.000000 0.500000)"));
        assert!(svg.find("rotate(90.").unwrap() < svg.find("<circle r=\"25\"").unwrap());
        s.actors[0].scale.x = f64::NAN;
        assert_eq!(render_svg(&s, 640, 360), Err(Error::InvalidActor(0)));
    }
    #[test]
    fn skeletal_head_and_limbs_are_rendered_from_evaluated_points() {
        let mut s = scene();
        s.actors[0].skeleton[HEAD] = motion_scene::Vec2::new(4.0, -88.0);
        s.actors[0].skeleton[RIGHT_HAND] = motion_scene::Vec2::new(35.0, -70.0);
        let svg = render_svg(&s, 640, 360).unwrap();
        assert!(svg.contains("translate(4.000000 -88.000000)"));
        assert!(svg.contains("L35.000000 -70.000000"));
    }
    #[test]
    fn escapes_untrusted_labels_and_rejects_xml_controls() {
        let mut s = scene();
        s.actors[0].id = "<script>&\"'".into();
        let svg = render_svg(&s, 640, 360).unwrap();
        assert!(svg.contains("&lt;script&gt;&amp;&quot;&apos;"));
        assert!(!svg.contains("<script>"));
        s.actors[0].id.push('\0');
        assert_eq!(render_svg(&s, 640, 360), Err(Error::InvalidActor(0)));
    }
    #[test]
    fn visemes_change_mouth_without_changing_pose() {
        let s = scene();
        let svg = render_svg_with_mouths(&s, 640, 360, &[("EMMA", Viseme::A)]).unwrap();
        assert!(svg.contains("width=\"14\" height=\"14\""));
        assert!(!svg.contains("M-8 9L0 14L8 9"));
        assert_eq!(
            render_svg_with_mouths(&s, 640, 360, &[("OTHER", Viseme::A)]).unwrap(),
            render_svg(&s, 640, 360).unwrap()
        );
    }
    #[test]
    fn rejects_unbounded_or_nonfinite_inputs() {
        let mut s = scene();
        for dimension in [0, 8193] {
            assert_eq!(
                render_svg(&s, dimension, 360),
                Err(Error::InvalidDimensions)
            );
        }
        s.actors[0].skeleton[0].x = f64::NAN;
        assert_eq!(render_svg(&s, 640, 360), Err(Error::InvalidActor(0)));
        s = scene();
        s.actors[0].wave = f64::NAN;
        assert_eq!(render_svg(&s, 640, 360), Err(Error::InvalidActor(0)));
        s.actors[0].wave = 0.0;
        s.actors[0].x = f64::INFINITY;
        assert_eq!(render_svg(&s, 640, 360), Err(Error::InvalidActor(0)));
        s = scene();
        s.actors = vec![s.actors[0].clone(); MAX_ACTORS + 1];
        assert_eq!(render_svg(&s, 640, 360), Err(Error::TooManyActors));
    }
}

#[cfg(test)]
mod composition_tests {
    use super::*;
    #[test]
    fn evaluated_composition_has_clipped_camera_and_canvas_fade() {
        let scene = EvaluatedScene {
            time: 0.0,
            width: 640,
            height: 360,
            actors: vec![],
            camera: motion_scene::CameraPose {
                x: 50.0,
                y: 20.0,
                zoom: 2.0,
            },
            background_color: "#12abef".into(),
            opacity: 0.5,
        };
        let svg = render_svg(&scene, 640, 360).unwrap();
        assert!(svg.contains("translate(-420.000000 -220.000000) scale(2.000000)"));
        assert!(svg.contains("clip-path=\"url(#canvas)\" opacity=\"0.500000\""));
        assert!(svg.contains("fill=\"#12abef\""));
        assert_eq!(svg, include_str!("../tests/golden/composition.svg"));
        assert_eq!(svg, render_svg(&scene, 640, 360).unwrap());
        let mut invalid = scene;
        invalid.camera.zoom = f64::NAN;
        assert_eq!(
            render_svg(&invalid, 640, 360),
            Err(Error::InvalidComposition)
        );
        invalid.camera.zoom = 1.0;
        invalid.background_color = "red\"/>".into();
        assert_eq!(
            render_svg(&invalid, 640, 360),
            Err(Error::InvalidComposition)
        );
    }
}

#[cfg(test)]
mod timed_art_tests {
    use super::*;
    use motion_art::{Color, Content, Document, Layer, Plane, Rect, TimeRange};
    use std::collections::BTreeMap;

    fn layer(id: &str, plane: Plane, red: u8) -> Layer {
        Layer {
            id: id.into(),
            scene_id: "s".into(),
            name: id.into(),
            visible: true,
            opacity: 0.75,
            plane,
            active: Some(TimeRange {
                start: 1.0,
                end: 3.0,
            }),
            attachment: None,
            clip_shape: None,
            blend: motion_art::BlendMode::Normal,
            clip: Some(Rect {
                x: 1.0,
                y: 2.0,
                width: 30.0,
                height: 40.0,
            }),
            content: Content::Rectangle {
                rect: Rect {
                    x: 1.0,
                    y: 2.0,
                    width: 30.0,
                    height: 40.0,
                },
                color: Color {
                    r: red,
                    g: 0,
                    b: 0,
                    a: 255,
                },
            },
        }
    }

    #[test]
    fn artwork_planes_keep_document_order_and_share_camera_fade_and_clip() {
        let mut s = super::tests::scene();
        s.time = 1.0;
        s.camera = motion_scene::CameraPose {
            x: 50.0,
            y: 20.0,
            zoom: 2.0,
        };
        s.opacity = 0.5;
        // Interleaved document order must be stable independently in each plane.
        let mut attached = layer("fg2", Plane::Foreground, 4);
        attached.attachment = Some("EMMA".into());
        let doc = Document {
            layers: vec![
                layer("fg1", Plane::Foreground, 3),
                layer("bg1", Plane::Background, 1),
                attached,
                layer("bg2", Plane::Background, 2),
            ],
            ..Default::default()
        };
        let svg = render_svg_with_art(&s, 640, 360, &[], "s", &doc, &BTreeMap::new()).unwrap();
        let positions = [
            svg.find("fill=\"#181f2c\"").unwrap(),
            svg.find("translate(-420.000000 -220.000000) scale(2.000000)")
                .unwrap(),
            svg.find("fill=\"#010000\"").unwrap(),
            svg.find("fill=\"#020000\"").unwrap(),
            svg.find("stroke=\"#7eb9e2\"").unwrap(),
            svg.find("fill=\"#030000\"").unwrap(),
            svg.find("fill=\"#040000\"").unwrap(),
        ];
        assert!(positions.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(svg.contains("clip-path=\"url(#canvas)\" opacity=\"0.500000\""));
        assert!(svg.contains("translate(120.000000 250.000000)\" opacity=\"0.750000\""));
        for index in 0..4 {
            assert_eq!(svg.matches(&format!("id=\"art{index}\"")).count(), 1);
            assert_eq!(
                svg.matches(&format!("clip-path=\"url(#art{index})\""))
                    .count(),
                1
            );
        }
    }

    #[test]
    fn half_open_timing_overlap_and_random_seek_are_stateless() {
        let mut s = super::tests::scene();
        let mut later = layer("later", Plane::Foreground, 2);
        later.active = Some(TimeRange {
            start: 2.0,
            end: 4.0,
        });
        let mut hidden = layer("hidden", Plane::Background, 3);
        hidden.visible = false;
        let mut other_scene = layer("other", Plane::Background, 4);
        other_scene.scene_id = "other".into();
        let doc = Document {
            layers: vec![
                layer("first", Plane::Background, 1),
                later,
                hidden,
                other_scene,
            ],
            ..Default::default()
        };
        for (time, first, second) in [
            (2.5, true, true),
            (4.0, false, false),
            (1.0, true, false),
            (3.0, false, true),
            (0.999, false, false),
            (2.0, true, true),
        ] {
            s.time = time;
            let svg = render_svg_with_art(&s, 640, 360, &[], "s", &doc, &BTreeMap::new()).unwrap();
            assert_eq!(svg.contains("fill=\"#010000\""), first, "{time}");
            assert_eq!(svg.contains("fill=\"#020000\""), second, "{time}");
            assert!(!svg.contains("fill=\"#030000\""));
            assert!(!svg.contains("fill=\"#040000\""));
            if !first && !second {
                assert_eq!(svg, render_svg(&s, 640, 360).unwrap());
            }
        }
    }

    #[test]
    fn inactive_images_need_no_decoded_resource_and_invalid_time_is_rejected() {
        let mut s = super::tests::scene();
        let mut image = layer("image", Plane::Background, 1);
        image.content = Content::Image {
            asset_id: "asset".into(),
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
        };
        let doc = Document {
            layers: vec![image],
            ..Default::default()
        };
        assert!(render_svg_with_art(&s, 640, 360, &[], "s", &doc, &BTreeMap::new()).is_ok());
        s.time = 1.0;
        assert!(matches!(
            render_svg_with_art(&s, 640, 360, &[], "s", &doc, &BTreeMap::new()),
            Err(Error::InvalidArtwork(_))
        ));
        s.time = f64::NAN;
        assert_eq!(render_svg(&s, 640, 360), Err(Error::InvalidComposition));
    }

    #[test]
    fn painted_mask_preserves_base_order_dabs_and_shared_transforms() {
        use motion_art::{ClipShape, MaskStroke, Point};
        let mut scene = super::tests::scene();
        scene.time = 1.0;
        let mut item = layer("painted", Plane::Foreground, 255);
        item.clip = None;
        item.attachment = Some("EMMA".into());
        let dab = MaskStroke {
            points: vec![Point { x: 10.0, y: 20.0 }],
            radius: 3.0,
            erase: false,
            pressures: Vec::new(),
            hardness: 1.0,
        };
        item.clip_shape = Some(ClipShape::Painted {
            motion: Vec::new(),
            base_visible: false,
            strokes: vec![
                dab.clone(),
                MaskStroke {
                    erase: true,
                    radius: 2.0,
                    ..dab.clone()
                },
                MaskStroke {
                    points: vec![Point { x: 10.0, y: 20.0 }, Point { x: 15.0, y: 25.0 }],
                    ..dab
                },
            ],
        });
        let mut doc = Document {
            layers: vec![item],
            ..Default::default()
        };
        let render = |doc: &Document| {
            render_svg_with_art(&scene, 640, 360, &[], "s", doc, &BTreeMap::new()).unwrap()
        };
        let svg = render(&doc);
        assert!(svg.contains("mask-type=\"luminance\""));
        assert!(svg.contains("maskContentUnits=\"userSpaceOnUse\""));
        assert!(svg.contains("translate(120.000000 250.000000)\" opacity=\"0.750000\""));
        let base = svg.find("height=\"40\" fill=\"black\"").unwrap();
        let paint = svg.find("r=\"3\" fill=\"white\"").unwrap();
        let erase = svg.find("r=\"2\" fill=\"black\"").unwrap();
        let restore = svg.find("stroke=\"white\" stroke-width=\"6\"").unwrap();
        assert!(base < paint && paint < erase && erase < restore);
        assert!(svg.contains("stroke-linecap=\"round\" stroke-linejoin=\"round\""));
        assert_eq!(svg, render(&doc));
        if let Some(ClipShape::Painted {
            base_visible,
            strokes,
            ..
        }) = &mut doc.layers[0].clip_shape
        {
            *base_visible = true;
            strokes.clear();
        }
        let empty_visible = render(&doc);
        assert!(empty_visible.contains("height=\"40\" fill=\"white\""));
        assert!(!empty_visible.contains("<circle cx=\"10\" cy=\"20\""));
    }

    #[test]
    fn painted_mask_content_bounds_include_round_caps() {
        let content = Content::Stroke {
            points: vec![
                motion_art::Point { x: -10.0, y: -5.0 },
                motion_art::Point { x: 20.0, y: 15.0 },
            ],
            color: Color {
                r: 255,
                g: 0,
                b: 0,
                a: 255,
            },
            width: 8.0,
        };
        assert_eq!(
            artwork_content_bounds(&content),
            Rect {
                x: -14.0,
                y: -9.0,
                width: 38.0,
                height: 28.0,
            }
        );
    }
}
