//! Deterministic, self-contained SVG frames from evaluated scenes.
//! Uses original procedural characters; no fonts, network assets, or video encoder.
use motion_lipsync::Viseme;
use motion_scene::{EvaluatedScene, HIP, CHEST, HEAD, LEFT_HAND, RIGHT_HAND, LEFT_FOOT, RIGHT_FOOT};
use std::fmt::Write;

pub const MAX_DIMENSION: u32 = 8192;
pub const MAX_ACTORS: usize = 256;
const MAX_LABEL_BYTES: usize = 1024;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum Error {
    #[error("scene and output dimensions must be between 1 and 8192 pixels")]
    InvalidDimensions,
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
    if scene.actors.len() > MAX_ACTORS {
        return Err(Error::TooManyActors);
    }
    for (index, actor) in scene.actors.iter().enumerate() {
        if !actor.x.is_finite()
            || !actor.y.is_finite()
            || actor.x.abs() > 1_000_000.0
            || actor.y.abs() > 1_000_000.0
            || actor.skeleton.iter().any(|p| !p.x.is_finite() || !p.y.is_finite() || p.x.abs() > 1_000_000.0 || p.y.abs() > 1_000_000.0)
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
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\"><rect width=\"100%\" height=\"100%\" fill=\"#181f2c\"/><g transform=\"translate({ox:.6} {oy:.6}) scale({scale:.6})\">"
    );
    for actor in &scene.actors {
        write!(
            svg,
            "<g transform=\"translate({:.6} {:.6})\">",
            actor.x, actor.y
        )
        .unwrap();
        let p = &actor.skeleton;
        svg.push_str("<path d=\"");
        for (a,b) in [(HIP,CHEST),(CHEST,HEAD),(CHEST,LEFT_HAND),(CHEST,RIGHT_HAND),(HIP,LEFT_FOOT),(HIP,RIGHT_FOOT)] {
            write!(svg,"M{:.6} {:.6}L{:.6} {:.6}",p[a].x,p[a].y,p[b].x,p[b].y).unwrap();
        }
        svg.push_str("\" fill=\"none\" stroke=\"#7eb9e2\" stroke-width=\"5\" stroke-linecap=\"round\"/>");
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
            svg.push_str("<path d=\"M-8 9L0 14L8 9\" fill=\"none\" stroke=\"#7d2d39\" stroke-width=\"2\"/>");
        }
        if actor.emotion == "surprised" {
            svg.push_str("<path d=\"M-12 -13L-4 -15M4 -13L12 -15\" stroke=\"black\"/>");
        }
        svg.push_str("</g>");
        write!(svg, "<text y=\"45\" text-anchor=\"middle\" dominant-baseline=\"middle\" font-family=\"sans-serif\" font-size=\"12\" fill=\"white\">{} · {}</text></g>", escape(&actor.id), escape(&actor.emotion)).unwrap();
    }
    svg.push_str("</g></svg>\n");
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
            width: 640,
            height: 360,
            actors: vec![ActorPose {
                id: "EMMA".into(),
                x: 120.0,
                y: 250.0,
                emotion: "happy".into(),
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
        assert!(!svg.contains("M-8 -81"));
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
