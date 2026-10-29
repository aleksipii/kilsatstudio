//! Named display slots on rig parts.
//!
//! A slot turns a drawn screen (a phone, tablet, laptop, TV or billboard) into
//! a place where any image source can be shown. The slot is a parallelogram in
//! its glass part's joint space: `origin` is the content's top-left corner as
//! seen on the device, `x_axis` runs along its width and `y_axis` along its
//! height, so the slot's aspect ratio is `|x_axis| / |y_axis|`. Content is
//! clipped to the glass part's polygon, so the glass part's own corner
//! rounding (`smooth` or a chamfered polygon) rounds the picture too.
//!
//! The slot only says *where*; what is shown and when is decided by the scene
//! (`motion_script::ScreenCue`) and applied after posing by
//! `PreparedRig::apply_screen`, on the same joint transforms as every other
//! part, so the picture moves with the hand in preview and export alike.
use crate::{
    definition::valid_id, Affine2, Error, EvaluatedTexture, PreparedRig, RigDefinition, TextureFit,
    TextureGlow, Transform2, Vec2,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Upper bound on slots per rig.
pub const MAX_SCREEN_SLOTS: usize = 8;
/// Glass colour of a switched-off screen.
pub const SCREEN_OFF_COLOR: [u8; 4] = [12, 13, 16, 255];
/// Glass colour behind content, visible in `contain` letterbox bands.
pub const SCREEN_LETTERBOX_COLOR: [u8; 4] = [0, 0, 0, 255];
/// Highest accepted screen brightness multiplier.
pub const MAX_SCREEN_BRIGHTNESS: f64 = 4.0;
/// Largest accepted glow reach, in multiples of the slot's longer side.
pub const MAX_SCREEN_GLOW_RADIUS: f64 = 4.0;
/// Glow reach used when a cue sets `glow` without `glow_radius`.
pub const DEFAULT_SCREEN_GLOW_RADIUS: f64 = 0.75;

/// How brightly a slot shows its picture and how much it lights its
/// surroundings. `Off` screens never glow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenLight {
    /// Multiplier on the picture's colour, `0..=MAX_SCREEN_BRIGHTNESS`.
    pub brightness: f64,
    /// Glow opacity scale, `0..=1`; 0 draws no glow.
    pub glow: f64,
    /// Glow reach beyond the glass in multiples of the slot's longer side,
    /// `(0, MAX_SCREEN_GLOW_RADIUS]`.
    pub glow_radius: f64,
}
impl Default for ScreenLight {
    fn default() -> Self {
        Self {
            brightness: 1.0,
            glow: 0.0,
            glow_radius: DEFAULT_SCREEN_GLOW_RADIUS,
        }
    }
}
impl ScreenLight {
    /// True when every field is finite and within its documented range.
    pub fn is_valid(&self) -> bool {
        (0.0..=MAX_SCREEN_BRIGHTNESS).contains(&self.brightness)
            && (0.0..=1.0).contains(&self.glow)
            && self.glow_radius > 0.0
            && self.glow_radius <= MAX_SCREEN_GLOW_RADIUS
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenSlotDefinition {
    /// Name used by scenes, unique within the rig (for example `phone`).
    pub id: String,
    /// Rigid part whose polygon is the glass: content is drawn into and
    /// clipped by it.
    pub part: String,
    /// Content rectangle corner and edge vectors in the glass part's joint space.
    pub origin: [f64; 2],
    pub x_axis: [f64; 2],
    pub y_axis: [f64; 2],
    /// Parts drawn over the glass (the rig's own drawn UI). They are hidden
    /// while the slot shows content or is switched off.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub overlays: Vec<String>,
}
impl ScreenSlotDefinition {
    /// Slot width and height in joint units.
    pub fn size(&self) -> [f64; 2] {
        [
            self.x_axis[0].hypot(self.x_axis[1]),
            self.y_axis[0].hypot(self.y_axis[1]),
        ]
    }
    /// Slot space (`[0, w] x [0, h]`, y down) to joint space.
    fn frame(&self) -> Affine2 {
        let [w, h] = self.size();
        Affine2 {
            a: self.x_axis[0] / w,
            b: self.x_axis[1] / w,
            c: self.y_axis[0] / h,
            d: self.y_axis[1] / h,
            tx: self.origin[0],
            ty: self.origin[1],
        }
    }
}

/// What a slot shows at one instant.
#[derive(Debug, Clone, PartialEq)]
pub enum ScreenContent {
    /// The rig's own drawn screen, unchanged.
    Drawn,
    /// A dark, switched-off screen.
    Off,
    /// A project image asset (a still, a sequence frame or a decoded video frame).
    Image {
        asset: String,
        fit: TextureFit,
        light: ScreenLight,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct PreparedScreen {
    id: String,
    part: usize,
    joint: String,
    frame: Affine2,
    size: [f64; 2],
    overlays: Vec<usize>,
}

pub(crate) fn validate_slots(
    rig: &RigDefinition,
    joints: &BTreeMap<&str, usize>,
) -> Result<(), Error> {
    if rig.screens.len() > MAX_SCREEN_SLOTS {
        return Err(Error::InvalidDefinition);
    }
    let mut ids = BTreeSet::new();
    for slot in &rig.screens {
        let part = rig.parts.iter().find(|p| p.id == slot.part);
        let bounded = |v: &f64| v.is_finite() && v.abs() <= 1_000_000.0;
        let [w, h] = slot.size();
        let cross = slot.x_axis[0] * slot.y_axis[1] - slot.x_axis[1] * slot.y_axis[0];
        let overlays_ok = slot.overlays.len() <= 32
            && slot
                .overlays
                .iter()
                .all(|o| o != &slot.part && rig.parts.iter().any(|p| &p.id == o))
            && slot.overlays.iter().collect::<BTreeSet<_>>().len() == slot.overlays.len();
        if !valid_id(&slot.id)
            || !ids.insert(slot.id.as_str())
            || !part.is_some_and(|p| p.skin.is_empty() && joints.contains_key(p.joint.as_str()))
            || !slot
                .origin
                .iter()
                .chain(&slot.x_axis)
                .chain(&slot.y_axis)
                .all(bounded)
            || w < 1e-3
            || h < 1e-3
            // Non-degenerate and not mirrored: the picture must not read backwards.
            || cross <= 1e-6 * w * h
            || !overlays_ok
        {
            return Err(Error::InvalidDefinition);
        }
    }
    Ok(())
}

/// Resolve slots to part indices. Only called after `validate_slots` passed.
pub(crate) fn prepare_slots(rig: &RigDefinition) -> Vec<PreparedScreen> {
    let index = |id: &str| rig.parts.iter().position(|p| p.id == id).unwrap_or(0);
    rig.screens
        .iter()
        .map(|slot| PreparedScreen {
            id: slot.id.clone(),
            part: index(&slot.part),
            joint: rig.parts[index(&slot.part)].joint.clone(),
            frame: slot.frame(),
            size: slot.size(),
            overlays: slot.overlays.iter().map(|o| index(o)).collect(),
        })
        .collect()
}

impl PreparedRig {
    /// Ids of this rig's screen slots, in definition order.
    pub fn screen_ids(&self) -> impl Iterator<Item = &str> {
        self.screens().iter().map(|s| s.id.as_str())
    }
    /// Index of the joint that carries slot `id`'s glass part.
    pub fn screen_joint(&self, id: &str) -> Option<usize> {
        let screen = self.screens().iter().find(|s| s.id == id)?;
        self.joint_index(&screen.joint)
    }
    /// Show `content` in slot `id` on parts evaluated from this rig with the
    /// same joint `offsets`. `Drawn` leaves the parts untouched.
    pub fn apply_screen(
        &self,
        parts: &mut [crate::EvaluatedPart],
        offsets: &[(usize, Transform2)],
        id: &str,
        content: &ScreenContent,
    ) -> Result<(), Error> {
        let screen = self
            .screens()
            .iter()
            .find(|s| s.id == id)
            .ok_or(Error::InvalidDefinition)?;
        if parts.len() <= screen.part || screen.overlays.iter().any(|o| *o >= parts.len()) {
            return Err(Error::InvalidDefinition);
        }
        let (color, texture) = match content {
            ScreenContent::Drawn => return Ok(()),
            ScreenContent::Off => (SCREEN_OFF_COLOR, None),
            ScreenContent::Image { asset, fit, light } => {
                if asset.trim().is_empty() || asset.len() > 1024 || !light.is_valid() {
                    return Err(Error::InvalidDefinition);
                }
                let joint = self
                    .joint_index(&screen.joint)
                    .ok_or(Error::InvalidDefinition)?;
                let world = self.world_transforms(offsets)?;
                let [w, h] = screen.size;
                (
                    SCREEN_LETTERBOX_COLOR,
                    Some(EvaluatedTexture {
                        asset: asset.clone(),
                        uv: [0.0, 0.0, 1.0, 1.0],
                        rest_bounds: [0.0, 0.0, w, h],
                        transform: world[joint].compose(screen.frame),
                        fit: *fit,
                        brightness: light.brightness,
                        glow: (light.glow > 0.0).then(|| TextureGlow {
                            strength: light.glow,
                            radius: light.glow_radius * w.max(h),
                        }),
                    }),
                )
            }
        };
        let glass = &mut parts[screen.part];
        // A transparent glass stays transparent (for example a hidden variant).
        if glass.color[3] != 0 {
            glass.color = color;
            glass.texture = texture;
        }
        for overlay in &screen.overlays {
            let part = &mut parts[*overlay];
            part.color[3] = 0;
            part.texture = None;
            part.outline = None;
        }
        Ok(())
    }
    /// Actor-space corners (top-left, top-right, bottom-right, bottom-left) of
    /// slot `id` for the given joint offsets.
    pub fn screen_corners(
        &self,
        offsets: &[(usize, Transform2)],
        id: &str,
    ) -> Result<[Vec2; 4], Error> {
        let screen = self
            .screens()
            .iter()
            .find(|s| s.id == id)
            .ok_or(Error::InvalidDefinition)?;
        let joint = self
            .joint_index(&screen.joint)
            .ok_or(Error::InvalidDefinition)?;
        let m = self.world_transforms(offsets)?[joint].compose(screen.frame);
        let [w, h] = screen.size;
        Ok([
            m.point(Vec2::new(0.0, 0.0)),
            m.point(Vec2::new(w, 0.0)),
            m.point(Vec2::new(w, h)),
            m.point(Vec2::new(0.0, h)),
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{RigJointDefinition, RigidPartDefinition};

    fn part(id: &str, points: Vec<[f64; 2]>) -> RigidPartDefinition {
        RigidPartDefinition {
            id: id.into(),
            joint: "hand".into(),
            points,
            color: [200, 200, 200, 255],
            ..Default::default()
        }
    }
    fn rig() -> RigDefinition {
        RigDefinition {
            version: 1,
            id: "device".into(),
            joints: vec![RigJointDefinition {
                id: "hand".into(),
                parent: None,
                x: 10.0,
                y: 20.0,
                rotation: 0.0,
                scale_x: 1.0,
                scale_y: 1.0,
            }],
            parts: vec![
                part(
                    "glass",
                    vec![[0.0, 0.0], [9.0, 0.0], [9.0, 19.5], [0.0, 19.5]],
                ),
                part("ui", vec![[1.0, 1.0], [8.0, 1.0], [8.0, 3.0]]),
            ],
            screens: vec![ScreenSlotDefinition {
                id: "phone".into(),
                part: "glass".into(),
                origin: [0.0, 0.0],
                x_axis: [9.0, 0.0],
                y_axis: [0.0, 19.5],
                overlays: vec!["ui".into()],
            }],
            ..Default::default()
        }
    }

    #[test]
    fn slot_validation_rejects_bad_references_and_mirroring() {
        assert!(rig().validate().is_ok());
        let mut r = rig();
        r.screens[0].part = "missing".into();
        assert!(r.validate().is_err());
        let mut r = rig();
        r.screens[0].overlays = vec!["glass".into()];
        assert!(r.validate().is_err());
        let mut r = rig();
        r.screens[0].x_axis = [-9.0, 0.0];
        assert!(r.validate().is_err(), "mirrored slot");
        let mut r = rig();
        r.screens[0].y_axis = [f64::NAN, 1.0];
        assert!(r.validate().is_err());
        let mut r = rig();
        r.screens.push(r.screens[0].clone());
        assert!(r.validate().is_err(), "duplicate id");
        // Older rigs without slots keep their serialized form.
        let mut r = rig();
        r.screens.clear();
        assert!(!serde_json::to_string(&r).unwrap().contains("screens"));
    }

    #[test]
    fn content_follows_the_joint_and_hides_overlays() {
        let prepared = rig().prepare().unwrap();
        let offsets = [(
            0usize,
            Transform2 {
                translation: Vec2::new(5.0, 0.0),
                rotation: std::f64::consts::FRAC_PI_2,
                scale: Vec2::new(1.0, 1.0),
            },
        )];
        let mut parts = prepared.evaluate_parts(&offsets).unwrap();
        let content = ScreenContent::Image {
            asset: "screens/demo.png".into(),
            fit: TextureFit::Cover,
            light: ScreenLight::default(),
        };
        prepared
            .apply_screen(&mut parts, &offsets, "phone", &content)
            .unwrap();
        let texture = parts[0].texture.as_ref().unwrap();
        assert_eq!(parts[0].color, SCREEN_LETTERBOX_COLOR);
        assert_eq!(parts[1].color[3], 0);
        // The slot's corners are the glass polygon's corners.
        let corners = prepared.screen_corners(&offsets, "phone").unwrap();
        for (corner, point) in corners.iter().zip(&parts[0].points) {
            assert!((corner.x - point.x).abs() < 1e-9 && (corner.y - point.y).abs() < 1e-9);
        }
        let far = texture.transform.point(Vec2::new(9.0, 19.5));
        assert!((far.x - corners[2].x).abs() < 1e-9 && (far.y - corners[2].y).abs() < 1e-9);

        let mut off = prepared.evaluate_parts(&offsets).unwrap();
        prepared
            .apply_screen(&mut off, &offsets, "phone", &ScreenContent::Off)
            .unwrap();
        assert_eq!(off[0].color, SCREEN_OFF_COLOR);
        assert!(off[0].texture.is_none());
        let mut drawn = prepared.evaluate_parts(&offsets).unwrap();
        let before = drawn.clone();
        prepared
            .apply_screen(&mut drawn, &offsets, "phone", &ScreenContent::Drawn)
            .unwrap();
        assert_eq!(drawn, before);
        assert!(prepared
            .apply_screen(&mut drawn, &offsets, "tv", &ScreenContent::Off)
            .is_err());
    }

    #[test]
    fn brightness_and_glow_reach_the_texture_and_never_an_off_screen() {
        let prepared = rig().prepare().unwrap();
        let apply = |content: &ScreenContent| {
            let mut parts = prepared.evaluate_parts(&[]).unwrap();
            prepared
                .apply_screen(&mut parts, &[], "phone", content)
                .map(|()| parts[0].clone())
        };
        let image = |light: ScreenLight| ScreenContent::Image {
            asset: "a".into(),
            fit: TextureFit::Cover,
            light,
        };
        // Defaults: unchanged picture, no glow.
        let texture = apply(&image(ScreenLight::default()))
            .unwrap()
            .texture
            .unwrap();
        assert_eq!((texture.brightness, texture.glow), (1.0, None));
        // Glow radius is relative to the slot's longer side (19.5).
        let lit = ScreenLight {
            brightness: 1.5,
            glow: 0.6,
            glow_radius: 2.0,
        };
        let texture = apply(&image(lit)).unwrap().texture.unwrap();
        assert_eq!(texture.brightness, 1.5);
        assert_eq!(
            texture.glow,
            Some(TextureGlow {
                strength: 0.6,
                radius: 39.0
            })
        );
        assert_eq!(
            texture.glow.unwrap().ellipse(texture.rest_bounds),
            [4.5, 9.75, 43.5, 48.75]
        );
        // Off has no picture, hence no glow.
        let off = apply(&ScreenContent::Off).unwrap();
        assert!(off.texture.is_none());
        // Out-of-range or non-finite values are rejected, not clamped.
        for bad in [
            ScreenLight {
                brightness: -0.1,
                ..lit
            },
            ScreenLight {
                brightness: f64::NAN,
                ..lit
            },
            ScreenLight {
                brightness: MAX_SCREEN_BRIGHTNESS + 0.1,
                ..lit
            },
            ScreenLight { glow: 1.1, ..lit },
            ScreenLight {
                glow: f64::INFINITY,
                ..lit
            },
            ScreenLight {
                glow_radius: 0.0,
                ..lit
            },
            ScreenLight {
                glow_radius: f64::NAN,
                ..lit
            },
        ] {
            assert!(!bad.is_valid(), "{bad:?}");
            assert!(apply(&image(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn glow_paint_follows_the_mean_colour_and_skips_black() {
        let glow = TextureGlow {
            strength: 0.5,
            radius: 10.0,
        };
        let (rgb, alpha) = glow.paint([0.4, 0.2, 0.0], 1.0).unwrap();
        assert_eq!(rgb, [255, 128, 0]);
        let luma = 0.2126 * 0.4 + 0.7152 * 0.2;
        assert!((alpha - 0.5 * luma).abs() < 1e-12);
        // Brightness scales the glow too, clamped to opaque.
        let (_, brighter) = glow.paint([0.4, 0.2, 0.0], 2.0).unwrap();
        assert!((brighter - 2.0 * alpha).abs() < 1e-12);
        assert_eq!(glow.paint([1.0; 3], 4.0).unwrap().1, 1.0);
        assert_eq!(glow.paint([0.0; 3], 1.0), None);
        assert_eq!(glow.paint([0.5; 3], 0.0), None);
    }

    #[test]
    fn fit_modes_place_a_wide_recording_in_a_tall_slot() {
        let texture = |fit| EvaluatedTexture {
            asset: "a".into(),
            uv: [0.0, 0.0, 1.0, 1.0],
            rest_bounds: [0.0, 0.0, 9.0, 18.0],
            transform: Affine2 {
                a: 1.0,
                b: 0.0,
                c: 0.0,
                d: 1.0,
                tx: 0.0,
                ty: 0.0,
            },
            fit,
            brightness: 1.0,
            glow: None,
        };
        // 1:1 image into a 1:2 slot.
        assert_eq!(
            texture(TextureFit::Stretch).placement(100, 100),
            ([0.0, 0.0, 9.0, 18.0], [0.0, 0.0, 1.0, 1.0])
        );
        let (bounds, uv) = texture(TextureFit::Cover).placement(100, 100);
        assert_eq!(bounds, [0.0, 0.0, 9.0, 18.0]);
        assert!((uv[0] - 0.25).abs() < 1e-12 && (uv[2] - 0.75).abs() < 1e-12);
        assert_eq!([uv[1], uv[3]], [0.0, 1.0]);
        let (bounds, uv) = texture(TextureFit::Contain).placement(100, 100);
        assert_eq!(uv, [0.0, 0.0, 1.0, 1.0]);
        assert!((bounds[1] - 4.5).abs() < 1e-12 && (bounds[3] - 9.0).abs() < 1e-12);
        // A 19.5:9 portrait recording into a 2:1 slot: cover crops top and bottom.
        let (_, uv) = texture(TextureFit::Cover).placement(1179, 2556);
        assert!(uv[0] == 0.0 && uv[2] == 1.0 && uv[1] > 0.0 && uv[3] < 1.0);
    }
}
