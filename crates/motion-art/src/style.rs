//! Stick-figure background style check (docs/stick-studio-art-direction.md § 3).
//!
//! The art direction asks for muted large surfaces, dark limbs that stay
//! readable against them, and background lines calmer than the character
//! silhouette. This module turns those into measurable, deterministic rules
//! over one scene's background-plane layers. It reports; it never edits.

use crate::{Color, Content, Document, Plane};
use serde::{Deserialize, Serialize};

/// Limb width of the stick-studio rigs at actor scale 1 (thigh half-width 4.5).
pub const STICK_LIMB_WIDTH: f64 = 9.0;
/// Limb ink shared by the stick-studio rigs.
pub const STICK_LIMB_INK: Color = Color {
    r: 36,
    g: 40,
    b: 45,
    a: 255,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BackgroundStyle {
    pub canvas_width: f64,
    pub canvas_height: f64,
    /// On-screen limb width in scene pixels (rig width × actor scale).
    pub limb_width: f64,
    pub limb_ink: Color,
    /// Rectangles/ellipses covering at least this canvas fraction are "large".
    pub large_surface_fraction: f64,
    /// Maximum HSV saturation for large opaque surfaces.
    pub max_surface_saturation: f64,
    /// Minimum WCAG contrast ratio between a large surface and the limb ink.
    pub min_limb_contrast: f64,
    /// Background strokes may be at most this many limb widths wide.
    pub max_line_to_limb: f64,
}

impl BackgroundStyle {
    /// Stick-studio defaults for a canvas and the scene's actor scale.
    pub fn stick_studio(canvas_width: f64, canvas_height: f64, actor_scale: f64) -> Self {
        Self {
            canvas_width,
            canvas_height,
            limb_width: STICK_LIMB_WIDTH * actor_scale,
            limb_ink: STICK_LIMB_INK,
            large_surface_fraction: 0.20,
            max_surface_saturation: 0.35,
            min_limb_contrast: 4.5,
            max_line_to_limb: 2.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum StyleIssue {
    /// A large opaque surface is too saturated to stay in the background.
    SaturatedSurface { layer: String, saturation: f64 },
    /// A large opaque surface is too close in luminance to the limb ink.
    LowLimbContrast { layer: String, contrast: f64 },
    /// A background stroke is heavier than the allowed multiple of the limb width.
    HeavyLine {
        layer: String,
        width: f64,
        limit: f64,
    },
}

/// Check every visible background-plane layer of `scene_id`, in document order.
pub fn check_background(
    document: &Document,
    scene_id: &str,
    style: &BackgroundStyle,
) -> Vec<StyleIssue> {
    let canvas = style.canvas_width * style.canvas_height;
    let limb_luminance = relative_luminance(style.limb_ink);
    let line_limit = style.limb_width * style.max_line_to_limb;
    let mut issues = Vec::new();
    for layer in document
        .layers
        .iter()
        .filter(|l| l.visible && l.scene_id == scene_id && l.plane == Plane::Background)
    {
        match &layer.content {
            Content::Rectangle { rect, color } | Content::Ellipse { rect, color } => {
                let opaque = color.a == 255 && layer.opacity >= 1.0;
                if !opaque || canvas <= 0.0 {
                    continue;
                }
                // Clip to the canvas: an oversized wall counts as the whole canvas.
                let w = (rect.x + rect.width).min(style.canvas_width) - rect.x.max(0.0);
                let h = (rect.y + rect.height).min(style.canvas_height) - rect.y.max(0.0);
                if w <= 0.0 || h <= 0.0 || w * h / canvas < style.large_surface_fraction {
                    continue;
                }
                let saturation = saturation(*color);
                if saturation > style.max_surface_saturation {
                    issues.push(StyleIssue::SaturatedSurface {
                        layer: layer.id.clone(),
                        saturation,
                    });
                }
                let l = relative_luminance(*color);
                let contrast = (l.max(limb_luminance) + 0.05) / (l.min(limb_luminance) + 0.05);
                if contrast < style.min_limb_contrast {
                    issues.push(StyleIssue::LowLimbContrast {
                        layer: layer.id.clone(),
                        contrast,
                    });
                }
            }
            Content::Stroke { width, .. } if *width > line_limit => {
                issues.push(StyleIssue::HeavyLine {
                    layer: layer.id.clone(),
                    width: *width,
                    limit: line_limit,
                });
            }
            _ => {}
        }
    }
    issues
}

fn saturation(c: Color) -> f64 {
    let max = c.r.max(c.g).max(c.b);
    let min = c.r.min(c.g).min(c.b);
    if max == 0 {
        0.0
    } else {
        f64::from(max - min) / f64::from(max)
    }
}

/// WCAG 2 relative luminance of an sRGB colour.
fn relative_luminance(c: Color) -> f64 {
    let channel = |v: u8| {
        let v = f64::from(v) / 255.0;
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * channel(c.r) + 0.7152 * channel(c.g) + 0.0722 * channel(c.b)
}

/// Original character style directions (docs/model-sheet.md). `Default` keeps a
/// rig exactly as authored; the others restyle a copy of it. The snake_case
/// names are the values a screenplay `style:` key would use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CharacterStyle {
    #[default]
    Default,
    Stick,
    RubberHose,
    FlatTv,
    Geometric,
}

/// What a preset does with part outlines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutlineChoice {
    /// Keep each part's authored outline.
    Keep,
    /// One shared outer outline (silhouette mode) on every visible part.
    On { color: Color, width: f64 },
    /// Remove every outline.
    Off,
}

/// Replacement colours chosen by luminance role: dark inks, light paper and,
/// for mid tones, accents in order of first appearance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StylePalette {
    pub ink: Color,
    pub paper: Color,
    pub accents: [Color; 3],
}

/// Below this relative luminance a colour is ink; above `PAPER_LUMINANCE`, paper.
pub const INK_LUMINANCE: f64 = 0.08;
pub const PAPER_LUMINANCE: f64 = 0.6;

impl StylePalette {
    /// Map `source` colours (unique, in first-appearance order) to this palette.
    /// Each result keeps its source alpha.
    pub fn map(&self, source: &[Color]) -> Vec<Color> {
        let mut accent = 0;
        source
            .iter()
            .map(|c| {
                let l = relative_luminance(*c);
                let mut out = if l < INK_LUMINANCE {
                    self.ink
                } else if l > PAPER_LUMINANCE {
                    self.paper
                } else {
                    accent += 1;
                    self.accents[(accent - 1) % self.accents.len()]
                };
                out.a = c.a;
                out
            })
            .collect()
    }
}

/// Measurable settings of one style direction. `Default` changes nothing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CharacterStylePreset {
    pub style: CharacterStyle,
    /// Multiplier for the width of elongated (limb-like) parts.
    pub line_width_scale: f64,
    pub outline: OutlineChoice,
    /// Corner rounding samples for polygon parts (`None` keeps the authored value).
    pub joint_rounding: Option<u32>,
    pub palette: Option<StylePalette>,
    /// Default squash-and-stretch amount (0..=1). Recorded for the P03 animation
    /// principles; no evaluator reads it yet.
    pub squash_stretch: f64,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color { r, g, b, a: 255 }
}

impl CharacterStyle {
    pub const ALL: [CharacterStyle; 5] = [
        CharacterStyle::Default,
        CharacterStyle::Stick,
        CharacterStyle::RubberHose,
        CharacterStyle::FlatTv,
        CharacterStyle::Geometric,
    ];

    /// The snake_case name (`rubber_hose`, `flat_tv`, ...).
    pub fn name(self) -> &'static str {
        match self {
            CharacterStyle::Default => "default",
            CharacterStyle::Stick => "stick",
            CharacterStyle::RubberHose => "rubber_hose",
            CharacterStyle::FlatTv => "flat_tv",
            CharacterStyle::Geometric => "geometric",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|s| s.name() == name)
    }

    pub fn preset(self) -> CharacterStylePreset {
        let base = CharacterStylePreset {
            style: self,
            line_width_scale: 1.0,
            outline: OutlineChoice::Keep,
            joint_rounding: None,
            palette: None,
            squash_stretch: 0.0,
        };
        match self {
            CharacterStyle::Default => base,
            // Thin even ink lines, no outlines, lightly rounded joints.
            CharacterStyle::Stick => CharacterStylePreset {
                line_width_scale: 0.75,
                outline: OutlineChoice::Off,
                joint_rounding: Some(2),
                palette: Some(StylePalette {
                    ink: rgb(30, 30, 30),
                    paper: rgb(250, 250, 246),
                    accents: [rgb(120, 120, 120), rgb(170, 170, 170), rgb(90, 90, 90)],
                }),
                ..base
            },
            // Thick soft hose limbs, a black outer outline and strong squash.
            CharacterStyle::RubberHose => CharacterStylePreset {
                line_width_scale: 1.6,
                outline: OutlineChoice::On {
                    color: rgb(15, 12, 10),
                    width: 2.0,
                },
                joint_rounding: Some(8),
                palette: Some(StylePalette {
                    ink: rgb(15, 12, 10),
                    paper: rgb(246, 236, 214),
                    accents: [rgb(196, 64, 48), rgb(224, 178, 70), rgb(92, 120, 98)],
                }),
                squash_stretch: 0.35,
                ..base
            },
            // Flat fills with a bold outline and saturated broadcast colours.
            CharacterStyle::FlatTv => CharacterStylePreset {
                line_width_scale: 1.25,
                outline: OutlineChoice::On {
                    color: rgb(40, 32, 48),
                    width: 3.0,
                },
                joint_rounding: Some(3),
                palette: Some(StylePalette {
                    ink: rgb(40, 32, 48),
                    paper: rgb(255, 222, 190),
                    accents: [rgb(54, 132, 214), rgb(240, 112, 72), rgb(110, 196, 120)],
                }),
                squash_stretch: 0.1,
                ..base
            },
            // Sharp corners, no outline, a cool explainer palette.
            CharacterStyle::Geometric => CharacterStylePreset {
                line_width_scale: 1.0,
                outline: OutlineChoice::Off,
                joint_rounding: Some(0),
                palette: Some(StylePalette {
                    ink: rgb(28, 54, 78),
                    paper: rgb(236, 244, 248),
                    accents: [rgb(38, 166, 154), rgb(255, 183, 77), rgb(92, 107, 192)],
                }),
                ..base
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Layer, Point, Rect};

    fn layer(id: &str, plane: Plane, content: Content) -> Layer {
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
            content,
        }
    }
    fn rect(x: f64, y: f64, width: f64, height: f64) -> Rect {
        Rect {
            x,
            y,
            width,
            height,
        }
    }
    fn color(r: u8, g: u8, b: u8) -> Color {
        Color { r, g, b, a: 255 }
    }
    fn stroke(width: f64) -> Content {
        Content::Stroke {
            points: vec![Point { x: 0.0, y: 0.0 }, Point { x: 10.0, y: 0.0 }],
            color: color(40, 40, 40),
            width,
        }
    }

    #[test]
    fn muted_light_surfaces_and_calm_lines_pass() {
        let doc = Document {
            layers: vec![
                layer(
                    "wall",
                    Plane::Background,
                    Content::Rectangle {
                        rect: rect(0.0, 0.0, 1920.0, 1080.0),
                        color: color(131, 153, 164),
                    },
                ),
                layer("rail", Plane::Background, stroke(18.0)),
            ],
            ..Default::default()
        };
        let style = BackgroundStyle::stick_studio(1920.0, 1080.0, 1.0);
        assert_eq!(check_background(&doc, "s", &style), vec![]);
    }

    #[test]
    fn saturated_dark_surfaces_and_heavy_lines_are_reported() {
        let doc = Document {
            layers: vec![
                // Saturated red wall: fails saturation (and contrast, it is dark).
                layer(
                    "red_wall",
                    Plane::Background,
                    Content::Rectangle {
                        rect: rect(-100.0, -100.0, 4000.0, 4000.0),
                        color: color(120, 10, 10),
                    },
                ),
                // Dark grey floor: muted but too close to the limb ink.
                layer(
                    "dark_floor",
                    Plane::Background,
                    Content::Ellipse {
                        rect: rect(0.0, 540.0, 1920.0, 540.0),
                        color: color(50, 52, 55),
                    },
                ),
                layer("trunk", Plane::Background, stroke(28.0)),
                // Small surfaces, translucent glows and foreground are not checked.
                layer(
                    "badge",
                    Plane::Background,
                    Content::Rectangle {
                        rect: rect(0.0, 0.0, 100.0, 100.0),
                        color: color(255, 0, 0),
                    },
                ),
                layer(
                    "glow",
                    Plane::Background,
                    Content::Ellipse {
                        rect: rect(0.0, 0.0, 1920.0, 1080.0),
                        color: Color {
                            r: 255,
                            g: 200,
                            b: 0,
                            a: 24,
                        },
                    },
                ),
                layer("fg_line", Plane::Foreground, stroke(40.0)),
            ],
            ..Default::default()
        };
        let issues = check_background(
            &doc,
            "s",
            &BackgroundStyle::stick_studio(1920.0, 1080.0, 1.0),
        );
        let names: Vec<_> = issues
            .iter()
            .map(|i| match i {
                StyleIssue::SaturatedSurface { layer, .. } => ("saturated", layer.as_str()),
                StyleIssue::LowLimbContrast { layer, .. } => ("contrast", layer.as_str()),
                StyleIssue::HeavyLine { layer, .. } => ("heavy", layer.as_str()),
            })
            .collect();
        assert_eq!(
            names,
            [
                ("saturated", "red_wall"),
                ("contrast", "red_wall"),
                ("contrast", "dark_floor"),
                ("heavy", "trunk"),
            ]
        );
        // Larger actors raise the line limit with the on-screen limb width.
        let scaled = BackgroundStyle::stick_studio(1920.0, 1080.0, 1.6);
        assert!(!check_background(&doc, "s", &scaled)
            .iter()
            .any(|i| matches!(i, StyleIssue::HeavyLine { .. })));
    }

    #[test]
    fn luminance_and_saturation_reference_values() {
        assert_eq!(relative_luminance(color(255, 255, 255)), 1.0);
        assert_eq!(relative_luminance(color(0, 0, 0)), 0.0);
        assert_eq!(saturation(color(128, 128, 128)), 0.0);
        assert_eq!(saturation(color(255, 0, 0)), 1.0);
    }

    #[test]
    fn style_names_round_trip_and_default_is_identity() {
        for style in CharacterStyle::ALL {
            assert_eq!(CharacterStyle::from_name(style.name()), Some(style));
            let json = serde_json::to_string(&style).unwrap();
            assert_eq!(json, format!("\"{}\"", style.name()));
        }
        assert_eq!(CharacterStyle::from_name("anime"), None);
        let preset = CharacterStyle::Default.preset();
        assert_eq!(preset.line_width_scale, 1.0);
        assert_eq!(preset.outline, OutlineChoice::Keep);
        assert_eq!(preset.joint_rounding, None);
        assert_eq!(preset.palette, None);
        assert_eq!(preset.squash_stretch, 0.0);
        // Every non-default preset changes something measurable.
        for style in &CharacterStyle::ALL[1..] {
            assert_ne!(
                style.preset(),
                CharacterStylePreset {
                    style: *style,
                    ..preset
                }
            );
            let rounding = style.preset().joint_rounding.unwrap();
            assert!(rounding == 0 || (2..=16).contains(&rounding));
        }
    }

    #[test]
    fn palette_maps_by_luminance_role_and_keeps_alpha() {
        let palette = CharacterStyle::FlatTv.preset().palette.unwrap();
        let source = [
            color(36, 40, 45),
            color(249, 247, 239),
            color(183, 98, 87),
            Color {
                r: 143,
                g: 127,
                b: 109,
                a: 0,
            },
        ];
        let mapped = palette.map(&source);
        assert_eq!(mapped[0], palette.ink);
        assert_eq!(mapped[1], palette.paper);
        assert_eq!(mapped[2], palette.accents[0]);
        assert_eq!(
            mapped[3],
            Color {
                a: 0,
                ..palette.accents[1]
            }
        );
    }
}
