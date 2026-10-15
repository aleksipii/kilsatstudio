//! Deterministic, self-contained SVG frames from evaluated scenes.
//! Uses original procedural characters; no fonts, network assets, or video encoder.
use motion_lipsync::Viseme;
use motion_scene::{
    EvaluatedScene, CHEST, HEAD, HIP, LEFT_FOOT, LEFT_HAND, RIGHT_FOOT, RIGHT_HAND,
};
use std::fmt::Write;

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
    let scale = (width as f64 / scene.width as f64).min(height as f64 / scene.height as f64);
    let ox = (width as f64 - scene.width as f64 * scale) * 0.5;
    let oy = (height as f64 - scene.height as f64 * scale) * 0.5;
    let (camera_x, camera_y) = scene.camera.project(0.0, 0.0, scene.width, scene.height);
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><defs><clipPath id=\"canvas\"><rect width=\"{}\" height=\"{}\"/></clipPath></defs><rect width=\"100%\" height=\"100%\" fill=\"black\"/><g transform=\"translate({ox:.6} {oy:.6}) scale({scale:.6})\" clip-path=\"url(#canvas)\" opacity=\"{:.6}\"><rect width=\"{}\" height=\"{}\" fill=\"{}\"/><g transform=\"translate({camera_x:.6} {camera_y:.6}) scale({:.6})\">",
        scene.width, scene.height, scene.opacity, scene.width, scene.height, scene.background_color, scene.camera.zoom
    );
    for actor in &scene.actors {
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
        if actor.rotation != 0.0 || actor.scale.x != 1.0 || actor.scale.y != 1.0 {
            svg.push_str("</g>");
        }
        write!(svg, "<text y=\"45\" text-anchor=\"middle\" dominant-baseline=\"middle\" font-family=\"sans-serif\" font-size=\"12\" fill=\"white\">{} · {}</text></g>", escape(&actor.id), escape(&actor.emotion)).unwrap();
    }
    svg.push_str("</g></g></svg>\n");
    Ok(svg)
}

/// Foreground artwork in document order, inside the same camera, canvas clip and fade.
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
    let mut svg = render_svg_with_mouths(scene, width, height, mouths)?;
    let end = svg.len() - "</g></g></svg>\n".len();
    svg.truncate(end);
    use motion_art::Content;
    for (index, layer) in document
        .layers
        .iter()
        .enumerate()
        .filter(|(_, l)| l.scene_id == scene_id && l.visible)
    {
        let (x, y) = match &layer.attachment {
            Some(id) => match scene.actors.iter().find(|a| &a.id == id) {
                Some(a) => (a.x, a.y),
                None => continue,
            },
            None => (0.0, 0.0),
        };
        write!(
            svg,
            "<g transform=\"translate({x:.6} {y:.6})\" opacity=\"{:.6}\">",
            layer.opacity
        )
        .unwrap();
        if let Some(r) = layer.clip {
            write!(svg,"<defs><clipPath id=\"art{index}\"><rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/></clipPath></defs><g clip-path=\"url(#art{index})\">",r.x,r.y,r.width,r.height).unwrap();
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
        if layer.clip.is_some() {
            svg.push_str("</g>");
        }
        svg.push_str("</g>");
        if svg.len() > 48 * 1024 * 1024 {
            return Err(Error::InvalidArtwork(
                "SVG artwork exceeds 48 MiB frame budget".into(),
            ));
        }
    }
    svg.push_str("</g></g></svg>\n");
    Ok(svg)
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
    fn scene() -> EvaluatedScene {
        EvaluatedScene {
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
            }],
        }
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
