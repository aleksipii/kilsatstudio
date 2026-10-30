//! Multiplane parallax, aerial perspective and a value-structure check.
//!
//! Everything here is opt-in. The plain render entry points pass no
//! [`Multiplane`], and an empty one changes nothing, so existing output stays
//! byte-identical. See `docs/parallax.md`.
//!
//! * **Parallax.** Each artwork layer has a factor `p` (default 1.0, the
//!   character plane). The layer is drawn with [`layer_camera`]: the camera pan
//!   is scaled by `p` and the zoom is `1 + (zoom - 1) · p`. Far planes
//!   (`p` < 1) move less than near ones; `p` > 1 moves faster than the
//!   characters. The SVG painter and the GPU command builder both call
//!   [`Multiplane::camera_for`], so preview and export share one rule.
//!   Layers attached to an actor stay on the character plane (factor 1.0).
//! * **Aerial perspective.** With an [`Atmosphere`], a layer's vector colours
//!   lose saturation and blend toward the atmosphere colour by
//!   `strength · (1 − min(p, 1))`. The colours are rewritten on the CPU before
//!   either renderer sees the document, so both receive identical bytes.
//!   Image content cannot be recoloured this way and is reported in
//!   [`Hazed::skipped`] instead of being silently treated as hazed.
//! * **Value structure.** [`value_structure`] compares the luminance contrast
//!   of background artwork with that of the characters. It only warns.
//!
//! No function here reads a clock: camera and time come from the evaluated
//! scene, so a seek renders the same frame as frame-by-frame playback.

use motion_art::{Color, Content, Document, Layer, Plane};
use motion_scene::{CameraPose, EvaluatedScene};
use std::collections::BTreeMap;

/// Factor of the character plane and of every layer without an entry.
pub const CHARACTER_PARALLAX: f64 = 1.0;
/// Largest accepted factor (same bound as `motion_library::MAX_PARALLAX`).
pub const MAX_PARALLAX: f64 = 4.0;
/// Most layer entries one [`Multiplane`] accepts (matches the artwork budget).
pub const MAX_ENTRIES: usize = motion_art::MAX_LAYERS;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum ParallaxError {
    #[error("layer `{layer}`: parallax {value} must be finite and 0..=4")]
    InvalidFactor { layer: String, value: f64 },
    #[error("atmosphere strength {0} must be finite and 0..=1")]
    InvalidAtmosphere(f64),
    #[error("more than {MAX_ENTRIES} parallax entries")]
    TooManyEntries,
}

/// Aerial perspective: distant layers fade toward `color` and desaturate.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Atmosphere {
    pub color: Color,
    /// 0..=1; 0 leaves every colour unchanged.
    pub strength: f64,
}

/// Validated per-layer parallax factors plus an optional atmosphere.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Multiplane {
    factors: BTreeMap<String, f64>,
    atmosphere: Option<Atmosphere>,
}

impl Multiplane {
    /// `factors` maps document layer ids to parallax factors (for example
    /// `motion_library::Assembled::parallax`). Unknown ids are harmless.
    pub fn new(
        factors: impl IntoIterator<Item = (String, f64)>,
        atmosphere: Option<Atmosphere>,
    ) -> Result<Self, ParallaxError> {
        let mut map = BTreeMap::new();
        for (layer, value) in factors {
            if !value.is_finite() || !(0.0..=MAX_PARALLAX).contains(&value) {
                return Err(ParallaxError::InvalidFactor { layer, value });
            }
            map.insert(layer, value);
            if map.len() > MAX_ENTRIES {
                return Err(ParallaxError::TooManyEntries);
            }
        }
        if let Some(a) = atmosphere {
            if !a.strength.is_finite() || !(0.0..=1.0).contains(&a.strength) {
                return Err(ParallaxError::InvalidAtmosphere(a.strength));
            }
        }
        Ok(Self {
            factors: map,
            atmosphere,
        })
    }

    pub fn atmosphere(&self) -> Option<Atmosphere> {
        self.atmosphere
    }

    /// The factor of a layer id; [`CHARACTER_PARALLAX`] when it has no entry.
    pub fn factor(&self, layer_id: &str) -> f64 {
        self.factors
            .get(layer_id)
            .copied()
            .unwrap_or(CHARACTER_PARALLAX)
    }

    /// The camera a layer is drawn with. Attached layers follow their actor
    /// on the character plane and always use the scene camera.
    pub fn camera_for(&self, layer: &Layer, camera: CameraPose) -> CameraPose {
        if layer.attachment.is_some() {
            return camera;
        }
        layer_camera(camera, self.factor(&layer.id))
    }

    /// Haze amount of a layer, 0..=1: `strength · (1 − min(p, 1))`.
    pub fn haze(&self, layer_id: &str) -> f64 {
        let Some(a) = self.atmosphere else {
            return 0.0;
        };
        a.strength * (1.0 - self.factor(layer_id).min(1.0))
    }

    /// The document with aerial perspective applied to vector colours. Returns
    /// the input unchanged (no clone) when nothing is hazed.
    pub fn apply_atmosphere<'a>(&self, document: &'a Document) -> Hazed<'a> {
        let Some(atmosphere) = self.atmosphere else {
            return Hazed {
                document: std::borrow::Cow::Borrowed(document),
                skipped: Vec::new(),
            };
        };
        let mut skipped = Vec::new();
        let mut out: Option<Document> = None;
        for (index, layer) in document.layers.iter().enumerate() {
            let amount = if layer.attachment.is_some() {
                0.0
            } else {
                self.haze(&layer.id)
            };
            if amount == 0.0 {
                continue;
            }
            let hazed = |c: Color| hazed_color(c, atmosphere.color, amount);
            let content = match &layer.content {
                Content::Rectangle { rect, color } => Content::Rectangle {
                    rect: *rect,
                    color: hazed(*color),
                },
                Content::Ellipse { rect, color } => Content::Ellipse {
                    rect: *rect,
                    color: hazed(*color),
                },
                Content::Stroke {
                    points,
                    color,
                    width,
                } => Content::Stroke {
                    points: points.clone(),
                    color: hazed(*color),
                    width: *width,
                },
                Content::Image { .. } | Content::AnimatedImage { .. } => {
                    skipped.push(layer.id.clone());
                    continue;
                }
            };
            out.get_or_insert_with(|| document.clone()).layers[index].content = content;
        }
        Hazed {
            document: match out {
                Some(d) => std::borrow::Cow::Owned(d),
                None => std::borrow::Cow::Borrowed(document),
            },
            skipped,
        }
    }
}

/// Result of [`Multiplane::apply_atmosphere`].
#[derive(Debug, Clone)]
pub struct Hazed<'a> {
    pub document: std::borrow::Cow<'a, Document>,
    /// Ids of image layers that should have been hazed but were left as is.
    pub skipped: Vec<String>,
}

/// The camera seen by a plane with parallax `factor`. Factor 1.0 returns the
/// camera unchanged bit for bit, which keeps legacy output identical.
pub fn layer_camera(camera: CameraPose, factor: f64) -> CameraPose {
    if factor == CHARACTER_PARALLAX {
        return camera;
    }
    CameraPose {
        x: camera.x * factor,
        y: camera.y * factor,
        // A convex/extrapolated blend of 1 and zoom, clamped to the renderer's
        // valid zoom range.
        zoom: (1.0 + (camera.zoom - 1.0) * factor).clamp(0.05, 20.0),
        angle: Default::default(),
    }
}

/// Desaturate toward luma, then blend toward `fog`, both by `amount` (0..=1).
/// Alpha is kept. Rounds to the nearest 8-bit value.
pub fn hazed_color(color: Color, fog: Color, amount: f64) -> Color {
    let amount = amount.clamp(0.0, 1.0);
    let [r, g, b] = [color.r, color.g, color.b].map(f64::from);
    let luma = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let channel = |c: f64, f: u8| {
        let grey = c + (luma - c) * amount;
        (grey + (f64::from(f) - grey) * amount)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    Color {
        r: channel(r, fog.r),
        g: channel(g, fog.g),
        b: channel(b, fog.b),
        a: color.a,
    }
}

/// SVG transform placing `layer` under its parallax camera while it is already
/// inside the scene camera group: `translate((a_l − a_c)/z_c) scale(z_l/z_c)`,
/// where `a` is the projected origin. `None` when the cameras are equal.
pub(crate) fn svg_compensation(
    scene: &EvaluatedScene,
    layer: &Layer,
    multiplane: Option<&Multiplane>,
) -> Option<String> {
    let camera = multiplane?.camera_for(layer, scene.camera);
    if camera == scene.camera {
        return None;
    }
    let (cx, cy) = scene.camera.project(0.0, 0.0, scene.width, scene.height);
    let (lx, ly) = camera.project(0.0, 0.0, scene.width, scene.height);
    let z = scene.camera.zoom;
    Some(format!(
        "translate({:.6} {:.6}) scale({:.6})",
        (lx - cx) / z,
        (ly - cy) / z,
        camera.zoom / z
    ))
}

/// [`crate::render_svg_with_art`] with multiplane parallax and aerial
/// perspective. An empty `multiplane` gives byte-identical output.
#[allow(clippy::too_many_arguments)]
pub fn render_svg_with_multiplane(
    scene: &EvaluatedScene,
    width: u32,
    height: u32,
    mouths: &[(&str, motion_lipsync::Viseme)],
    scene_id: &str,
    document: &Document,
    images: &BTreeMap<String, motion_art::ImageResource>,
    multiplane: &Multiplane,
) -> Result<String, crate::Error> {
    document
        .validate()
        .map_err(|e| crate::Error::InvalidArtwork(e.to_string()))?;
    let hazed = multiplane.apply_atmosphere(document);
    crate::render_scene(
        scene,
        width,
        height,
        mouths,
        Some((scene_id, &hazed.document, images)),
        true,
        Some(multiplane),
    )
}

/// WCAG relative luminance of an sRGB colour, 0..=1.
pub fn relative_luminance(c: Color) -> f64 {
    let lin = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.04045 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b)
}

/// Luminance contrast of background artwork versus characters at one frame.
/// Contrast is the WCAG ratio `(L_max + 0.05) / (L_min + 0.05)` over the
/// opaque-enough (alpha ≥ 32) flat colours of each group. A heuristic for
/// staging, not a perceptual measurement.
#[derive(Debug, Clone, PartialEq)]
pub struct ValueReport {
    /// `None` when the background has no measurable flat colour.
    pub background: Option<f64>,
    /// `None` when no actor has a measurable colour.
    pub characters: Option<f64>,
    /// Background layers whose content (images) was not measured.
    pub unmeasured: Vec<String>,
}

impl ValueReport {
    /// True when the background is more contrasty than the characters.
    pub fn background_overpowers(&self) -> bool {
        matches!((self.background, self.characters), (Some(b), Some(c)) if b > c)
    }
}

/// Procedural actor colours drawn by `render_svg` (stroke, head, eyes, mouth).
const PROCEDURAL_COLORS: [[u8; 3]; 4] = [
    [0x7e, 0xb9, 0xe2],
    [0xf5, 0xc8, 0xa2],
    [0, 0, 0],
    [0x7d, 0x2d, 0x39],
];

/// Compare background and character contrast for `scene` (after aerial
/// perspective when `multiplane` has one). Never fails rendering; it only
/// reports.
pub fn value_structure(
    scene: &EvaluatedScene,
    scene_id: &str,
    document: &Document,
    multiplane: Option<&Multiplane>,
) -> Result<ValueReport, crate::Error> {
    let hazed = match multiplane {
        Some(m) => m.apply_atmosphere(document),
        None => Hazed {
            document: std::borrow::Cow::Borrowed(document),
            skipped: Vec::new(),
        },
    };
    let steps = hazed
        .document
        .composition(
            scene_id,
            Plane::Background,
            scene.time,
            Some(scene.camera.angle.name()),
        )
        .map_err(|e| crate::Error::InvalidArtwork(e.to_string()))?;
    let mut background = Vec::new();
    let mut unmeasured = Vec::new();
    for step in steps {
        let motion_art::CompositionStep::Layer(layer) = step else {
            continue;
        };
        if layer.attachment.is_some() {
            continue;
        }
        match &layer.content {
            Content::Rectangle { color, .. }
            | Content::Ellipse { color, .. }
            | Content::Stroke { color, .. } => {
                if color.a >= 32 && layer.opacity > 0.0 {
                    background.push(relative_luminance(*color));
                }
            }
            Content::Image { .. } | Content::AnimatedImage { .. } => {
                unmeasured.push(layer.id.clone())
            }
        }
    }
    let mut characters = Vec::new();
    for actor in &scene.actors {
        if actor.rigid_parts.is_empty() {
            characters.extend(
                PROCEDURAL_COLORS.map(|[r, g, b]| relative_luminance(Color { r, g, b, a: 255 })),
            );
        } else {
            characters.extend(
                actor
                    .rigid_parts
                    .iter()
                    .filter(|p| p.color[3] >= 32)
                    .map(|p| {
                        relative_luminance(Color {
                            r: p.color[0],
                            g: p.color[1],
                            b: p.color[2],
                            a: p.color[3],
                        })
                    }),
            );
        }
    }
    let ratio = |values: &[f64]| {
        let (lo, hi) = values
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &v| {
                (lo.min(v), hi.max(v))
            });
        (!values.is_empty()).then(|| (hi + 0.05) / (lo + 0.05))
    };
    Ok(ValueReport {
        background: ratio(&background),
        characters: ratio(&characters),
        unmeasured,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use motion_art::{Rect, MAX_LAYERS};

    fn rect_layer(id: &str, plane: Plane, x: f64, color: Color) -> Layer {
        Layer {
            id: id.into(),
            scene_id: "s".into(),
            name: id.into(),
            visible: true,
            plane,
            active: None,
            opacity: 1.0,
            attachment: None,
            clip: None,
            clip_shape: None,
            blend: Default::default(),
            content: Content::Rectangle {
                rect: Rect {
                    x,
                    y: 100.0,
                    width: 20.0,
                    height: 20.0,
                },
                color,
            },
            angle: None,
        }
    }
    const GREY: Color = Color {
        r: 120,
        g: 120,
        b: 120,
        a: 255,
    };
    const RED: Color = Color {
        r: 200,
        g: 40,
        b: 30,
        a: 255,
    };
    const SKY: Color = Color {
        r: 200,
        g: 215,
        b: 235,
        a: 255,
    };

    fn script(camera: &str) -> motion_script::Script {
        motion_script::parse(&format!(
            "project: {{title: p, fps: 24, width: 640, height: 360}}\n\
             characters: {{ALEX: {{asset: a}}}}\n\
             scenes:\n  - id: s\n    duration: 2\n    composition: {{camera: {camera}}}\n    events:\n      \
             - {{type: move, actor: ALEX, at: 0, x: 320, y: 250, duration: 1}}\n"
        ))
        .unwrap()
    }
    fn document() -> Document {
        Document {
            layers: vec![
                rect_layer("far", Plane::Background, 100.0, GREY),
                rect_layer("mid", Plane::Background, 200.0, GREY),
                rect_layer("near", Plane::Foreground, 300.0, RED),
            ],
            ..Default::default()
        }
    }
    fn multiplane(atmosphere: Option<Atmosphere>) -> Multiplane {
        Multiplane::new(
            [
                ("far".into(), 0.3),
                ("mid".into(), 0.6),
                ("near".into(), 1.0),
            ],
            atmosphere,
        )
        .unwrap()
    }
    fn svg(scene: &EvaluatedScene, mp: Option<&Multiplane>) -> String {
        let images = BTreeMap::new();
        match mp {
            Some(mp) => {
                render_svg_with_multiplane(scene, 640, 360, &[], "s", &document(), &images, mp)
            }
            None => crate::render_svg_with_art(scene, 640, 360, &[], "s", &document(), &images),
        }
        .unwrap()
    }

    #[test]
    fn validation_rejects_bad_factors_and_strength() {
        for value in [f64::NAN, -0.01, 4.01, f64::INFINITY] {
            assert!(matches!(
                Multiplane::new([("a".into(), value)], None),
                Err(ParallaxError::InvalidFactor { .. })
            ));
        }
        let fog = |strength| Atmosphere {
            color: SKY,
            strength,
        };
        for s in [f64::NAN, -0.1, 1.1] {
            assert!(matches!(
                Multiplane::new([], Some(fog(s))),
                Err(ParallaxError::InvalidAtmosphere(_))
            ));
        }
        let many = (0..=MAX_LAYERS).map(|i| (format!("l{i}"), 0.5));
        assert_eq!(
            Multiplane::new(many, None),
            Err(ParallaxError::TooManyEntries)
        );
        assert!(Multiplane::new([("a".into(), 0.0), ("b".into(), 4.0)], Some(fog(1.0))).is_ok());
    }

    #[test]
    fn layer_camera_scales_pan_and_zoom_and_keeps_identity_exact() {
        let camera = CameraPose {
            x: 100.0,
            y: -40.0,
            zoom: 2.0,
            angle: Default::default(),
        };
        assert_eq!(layer_camera(camera, 1.0), camera);
        let far = layer_camera(camera, 0.3);
        assert_eq!((far.x, far.y), (30.0, -12.0));
        assert!((far.zoom - 1.3).abs() < 1e-12);
        assert_eq!(layer_camera(camera, 0.0), CameraPose::default());
        // Extrapolation is clamped to the valid zoom range.
        let wide = CameraPose {
            x: 0.0,
            y: 0.0,
            zoom: 0.1,
            angle: Default::default(),
        };
        assert_eq!(layer_camera(wide, 4.0).zoom, 0.05);
    }

    #[test]
    fn stationary_camera_and_empty_multiplane_change_nothing() {
        // Default camera: every plane sees the same camera, so the SVG bytes
        // equal the legacy renderer's.
        let still = motion_scene::evaluate(&script("[]"), 0, 0.5).unwrap();
        assert_eq!(svg(&still, Some(&multiplane(None))), svg(&still, None));
        // A moving camera with no parallax entries is also unchanged.
        let moving = motion_scene::evaluate(
            &script("[{at: 0, x: 0, y: 0, zoom: 1}, {at: 2, x: 200, y: 0, zoom: 1.5}]"),
            0,
            1.0,
        )
        .unwrap();
        assert_eq!(
            svg(&moving, Some(&Multiplane::default())),
            svg(&moving, None)
        );
        // Zero-strength atmosphere is also unchanged.
        let none = Multiplane::new(
            [],
            Some(Atmosphere {
                color: SKY,
                strength: 0.0,
            }),
        )
        .unwrap();
        assert_eq!(svg(&moving, Some(&none)), svg(&moving, None));
    }

    /// Screen x of a layer-local point under its parallax camera.
    fn screen_x(scene: &EvaluatedScene, mp: &Multiplane, layer: &Layer, x: f64) -> f64 {
        mp.camera_for(layer, scene.camera)
            .project(x, 0.0, scene.width, scene.height)
            .0
    }

    #[test]
    fn pan_moves_far_three_tenths_of_near() {
        let script = script("[{at: 0, x: 0, y: 0, zoom: 1}, {at: 2, x: 200, y: 0, zoom: 1}]");
        let a = motion_scene::evaluate(&script, 0, 0.0).unwrap();
        let b = motion_scene::evaluate(&script, 0, 2.0).unwrap();
        let mp = multiplane(None);
        let doc = document();
        let shift = |layer: &Layer| screen_x(&b, &mp, layer, 0.0) - screen_x(&a, &mp, layer, 0.0);
        let (far, mid, near) = (
            shift(&doc.layers[0]),
            shift(&doc.layers[1]),
            shift(&doc.layers[2]),
        );
        assert_eq!(near, -200.0);
        assert!((far - 0.3 * near).abs() < 1e-9, "{far} vs {near}");
        assert!((mid - 0.6 * near).abs() < 1e-9, "{mid} vs {near}");
        // An attached layer follows its actor on the character plane.
        let mut held = doc.layers[0].clone();
        held.attachment = Some("ALEX".into());
        assert_eq!(shift(&held), near);
        // The SVG wraps exactly the non-unit layers, with the compensating transform.
        let out = svg(&b, Some(&mp));
        assert_eq!(
            out.matches("scale(1.000000)\"><g transform=\"translate(0")
                .count(),
            2
        );
        assert!(out.contains("<g transform=\"translate(140.000000 0.000000) scale(1.000000)\">"));
        assert!(out.contains("<g transform=\"translate(80.000000 0.000000) scale(1.000000)\">"));
    }

    #[test]
    fn seek_equals_frame_by_frame_playback() {
        let script = script("[{at: 0, x: 0, y: 0, zoom: 1}, {at: 2, x: 240, y: -30, zoom: 1.8}]");
        let mp = multiplane(Some(Atmosphere {
            color: SKY,
            strength: 0.5,
        }));
        let frames: Vec<String> = (0..=48)
            .map(|f| {
                svg(
                    &motion_scene::evaluate(&script, 0, f as f64 / 24.0).unwrap(),
                    Some(&mp),
                )
            })
            .collect();
        for f in [37usize, 3, 48, 0, 20, 37] {
            let scene = motion_scene::evaluate(&script, 0, f as f64 / 24.0).unwrap();
            assert_eq!(svg(&scene, Some(&mp)), frames[f], "frame {f}");
        }
    }

    #[test]
    fn atmosphere_hazes_by_distance_and_reports_images() {
        let mp = multiplane(Some(Atmosphere {
            color: SKY,
            strength: 1.0,
        }));
        assert!((mp.haze("far") - 0.7).abs() < 1e-12);
        assert!((mp.haze("mid") - 0.4).abs() < 1e-12);
        assert_eq!(mp.haze("near"), 0.0);
        let mut doc = document();
        doc.layers[0].content = Content::Rectangle {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            color: RED,
        };
        doc.layers.push(Layer {
            content: Content::Image {
                asset_id: "img".into(),
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 1.0,
                    height: 1.0,
                },
            },
            ..rect_layer("far_photo", Plane::Background, 0.0, GREY)
        });
        let mp = Multiplane::new(
            [
                ("far".into(), 0.3),
                ("mid".into(), 0.6),
                ("near".into(), 1.0),
                ("far_photo".into(), 0.3),
            ],
            mp.atmosphere(),
        )
        .unwrap();
        let hazed = mp.apply_atmosphere(&doc);
        assert_eq!(hazed.skipped, ["far_photo"]);
        let color = |d: &Document, i: usize| match d.layers[i].content {
            Content::Rectangle { color, .. } => color,
            _ => unreachable!(),
        };
        let far = color(&hazed.document, 0);
        // Lighter, closer to the sky and less saturated than the source red.
        let sat = |c: Color| c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b);
        assert!(sat(far) < sat(RED) / 2, "{far:?}");
        assert!(relative_luminance(far) > relative_luminance(RED));
        assert_eq!(far, hazed_color(RED, SKY, 0.7));
        // Near (factor 1) keeps its exact colour.
        assert_eq!(color(&hazed.document, 2), RED);
        // Endpoints: 0 is identity, 1 is the fog colour.
        assert_eq!(hazed_color(RED, SKY, 0.0), RED);
        assert_eq!(hazed_color(RED, SKY, 1.0), SKY);
        // No atmosphere borrows the input.
        assert!(matches!(
            multiplane(None).apply_atmosphere(&doc).document,
            std::borrow::Cow::Borrowed(_)
        ));
    }

    #[test]
    fn value_structure_warns_when_background_overpowers() {
        let scene = motion_scene::evaluate(&script("[]"), 0, 0.5).unwrap();
        let calm = document();
        let report = value_structure(&scene, "s", &calm, None).unwrap();
        assert_eq!(report.background, Some(1.0));
        assert!(!report.background_overpowers());
        let mut harsh = calm.clone();
        harsh.layers[0].content = Content::Rectangle {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            color: Color {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
        };
        harsh.layers[1].content = Content::Rectangle {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            color: Color {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
        };
        let report = value_structure(&scene, "s", &harsh, None).unwrap();
        assert!(report.background_overpowers(), "{report:?}");
        // Strong atmosphere pulls the far/mid planes toward the sky and fixes it.
        let fog = multiplane(Some(Atmosphere {
            color: SKY,
            strength: 1.0,
        }));
        let report = value_structure(&scene, "s", &harsh, Some(&fog)).unwrap();
        assert!(!report.background_overpowers(), "{report:?}");
    }
}
