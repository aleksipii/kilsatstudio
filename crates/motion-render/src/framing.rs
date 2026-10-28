//! Framing rules for wide and close shots, checked on an evaluated scene.
//!
//! This module only *checks* framing: it projects actor geometry through the
//! shared `CameraPose::project` rule and reports subjects that leave the frame
//! or its safety margin. Choosing camera keys stays with the authoring tools
//! (`motion_script::expand_shots`, the desktop shot-size helper). See
//! `docs/render-contract.md` § Framing.

use motion_scene::{ActorPose, EvaluatedScene, Vec2, HEAD, LEFT_HAND, RIGHT_HAND};

/// Procedural head radius and limb stroke, matching `render_svg`.
const PROCEDURAL_HEAD_RADIUS: f64 = 25.0;
const PROCEDURAL_STROKE: f64 = 5.0;

/// Axis-aligned rectangle; `min` is top-left in the coordinate space it names.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Bounds {
    pub min: Vec2,
    pub max: Vec2,
}

impl Bounds {
    fn from_points(points: impl IntoIterator<Item = Vec2>) -> Option<Self> {
        let mut points = points.into_iter();
        let first = points.next()?;
        let mut b = Self {
            min: first,
            max: first,
        };
        for p in points {
            b.min.x = b.min.x.min(p.x);
            b.min.y = b.min.y.min(p.y);
            b.max.x = b.max.x.max(p.x);
            b.max.y = b.max.y.max(p.y);
        }
        [b.min.x, b.min.y, b.max.x, b.max.y]
            .iter()
            .all(|v| v.is_finite())
            .then_some(b)
    }
    fn corners(self) -> [Vec2; 4] {
        [
            self.min,
            Vec2::new(self.max.x, self.min.y),
            self.max,
            Vec2::new(self.min.x, self.max.y),
        ]
    }
    fn expand(self, by: f64) -> Self {
        Self {
            min: Vec2::new(self.min.x - by, self.min.y - by),
            max: Vec2::new(self.max.x + by, self.max.y + by),
        }
    }
    pub fn width(&self) -> f64 {
        self.max.x - self.min.x
    }
    pub fn height(&self) -> f64 {
        self.max.y - self.min.y
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hand {
    Left,
    Right,
}

/// What must be visible. Custom rigs are matched by stick-studio part IDs.
#[derive(Debug, Clone, PartialEq)]
pub enum Subject {
    /// Every visible part of the actor (procedural skeleton and head).
    Actor(String),
    /// Custom rig part `face` (or `head`); procedural head circle.
    Head(String),
    /// Custom rig part `hand_l`/`hand_r`; procedural hand joint plus stroke.
    Hand(String, Hand),
    /// Catalog prop held in this hand: every visible rig part attached by
    /// `motion_rig::props::attach_hand_prop` (ids `heldprop_l_*`/`heldprop_r_*`).
    HeldProp(String, Hand),
    /// Arbitrary world-space (pre-camera) bounds.
    World(Bounds),
}

/// Shot scale and its margins, as fractions of the frame's shorter side.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FramingRule {
    /// Clear space required between each subject and the frame edge.
    pub margin: f64,
    /// Minimum height of the *first* subject as a fraction of frame height.
    pub min_primary_height: f64,
}

impl FramingRule {
    /// Wide: whole subjects inside a 4 % margin; no minimum size.
    pub const WIDE: Self = Self {
        margin: 0.04,
        min_primary_height: 0.0,
    };
    /// Close-up: subjects inside an 8 % margin; the primary subject (normally
    /// a head) fills at least 20 % of the frame height.
    pub const CLOSE_UP: Self = Self {
        margin: 0.08,
        min_primary_height: 0.20,
    };
}

#[derive(Debug, Clone, PartialEq)]
pub enum FramingViolation {
    /// The subject's actor is not in the evaluated scene (missing or hidden).
    MissingActor(Subject),
    /// The custom rig has no part the subject needs (e.g. no `face`).
    MissingPart(Subject),
    /// Projected subject bounds cross the margin; `frame` is the allowed area.
    OutsideMargin {
        subject: Subject,
        bounds: Bounds,
        frame: Bounds,
    },
    /// Primary subject smaller than the rule's minimum height fraction.
    TooSmall { subject: Subject, fraction: f64 },
}

/// World-space bounds of a subject in the evaluated scene, before the camera.
pub fn subject_world_bounds(
    scene: &EvaluatedScene,
    subject: &Subject,
) -> Result<Bounds, FramingViolation> {
    let actor_of = |id: &str| {
        scene
            .actors
            .iter()
            .find(|a| a.id == id)
            .ok_or_else(|| FramingViolation::MissingActor(subject.clone()))
    };
    let missing = || FramingViolation::MissingPart(subject.clone());
    match subject {
        Subject::World(bounds) => Ok(*bounds),
        Subject::Actor(id) => {
            let actor = actor_of(id)?;
            if actor.rigid_parts.is_empty() {
                let p = &actor.skeleton;
                let head = circle(p[HEAD], PROCEDURAL_HEAD_RADIUS);
                let local = Bounds::from_points(p.iter().copied().chain(head.corners()))
                    .ok_or_else(missing)?
                    .expand(PROCEDURAL_STROKE / 2.0);
                world(actor, local.corners()).ok_or_else(missing)
            } else {
                parts_world(actor, |_| true).ok_or_else(missing)
            }
        }
        Subject::Head(id) => {
            let actor = actor_of(id)?;
            if actor.rigid_parts.is_empty() {
                world(
                    actor,
                    circle(actor.skeleton[HEAD], PROCEDURAL_HEAD_RADIUS).corners(),
                )
                .ok_or_else(missing)
            } else {
                parts_world(actor, |part| part == "face")
                    .or_else(|| parts_world(actor, |part| part == "head"))
                    .ok_or_else(missing)
            }
        }
        Subject::Hand(id, hand) => {
            let actor = actor_of(id)?;
            let (joint, part) = match hand {
                Hand::Left => (LEFT_HAND, "hand_l"),
                Hand::Right => (RIGHT_HAND, "hand_r"),
            };
            if actor.rigid_parts.is_empty() {
                world(
                    actor,
                    circle(actor.skeleton[joint], PROCEDURAL_STROKE / 2.0).corners(),
                )
                .ok_or_else(missing)
            } else {
                parts_world(actor, |p| p == part).ok_or_else(missing)
            }
        }
        Subject::HeldProp(id, hand) => {
            let actor = actor_of(id)?;
            let prefix = match hand {
                Hand::Left => motion_rig::props::HandSide::Left,
                Hand::Right => motion_rig::props::HandSide::Right,
            }
            .part_prefix();
            parts_world(actor, |p| p.starts_with(prefix)).ok_or_else(missing)
        }
    }
}

/// Canvas-space bounds after the shared camera projection.
pub fn project(scene: &EvaluatedScene, world: Bounds) -> Bounds {
    let corner = |p: Vec2| {
        let (x, y) = scene.camera.project(p.x, p.y, scene.width, scene.height);
        Vec2::new(x, y)
    };
    Bounds {
        min: corner(world.min),
        max: corner(world.max),
    }
}

/// Check every subject against the rule; empty result means the shot passes.
/// The first subject is the primary one for `min_primary_height`.
pub fn check_framing(
    scene: &EvaluatedScene,
    subjects: &[Subject],
    rule: FramingRule,
) -> Vec<FramingViolation> {
    let (w, h) = (f64::from(scene.width), f64::from(scene.height));
    let m = rule.margin * w.min(h);
    let frame = Bounds {
        min: Vec2::new(m, m),
        max: Vec2::new(w - m, h - m),
    };
    let mut out = Vec::new();
    for (index, subject) in subjects.iter().enumerate() {
        let bounds = match subject_world_bounds(scene, subject) {
            Ok(b) => project(scene, b),
            Err(violation) => {
                out.push(violation);
                continue;
            }
        };
        if bounds.min.x < frame.min.x
            || bounds.min.y < frame.min.y
            || bounds.max.x > frame.max.x
            || bounds.max.y > frame.max.y
        {
            out.push(FramingViolation::OutsideMargin {
                subject: subject.clone(),
                bounds,
                frame,
            });
        }
        let fraction = bounds.height() / h;
        if index == 0 && fraction < rule.min_primary_height {
            out.push(FramingViolation::TooSmall {
                subject: subject.clone(),
                fraction,
            });
        }
    }
    out
}

fn circle(center: Vec2, r: f64) -> Bounds {
    Bounds {
        min: Vec2::new(center.x - r, center.y - r),
        max: Vec2::new(center.x + r, center.y + r),
    }
}

fn world(actor: &ActorPose, local: [Vec2; 4]) -> Option<Bounds> {
    Bounds::from_points(local.into_iter().map(|p| actor.local_to_world(p)))
}

/// Visible custom-rig parts in world space, including outline width.
fn parts_world(actor: &ActorPose, keep: impl Fn(&str) -> bool) -> Option<Bounds> {
    let mut all: Option<Bounds> = None;
    for part in actor
        .rigid_parts
        .iter()
        .filter(|p| p.color[3] != 0 && keep(&p.id))
    {
        let outline = part.outline.map_or(0.0, |o| o.width);
        let local = Bounds::from_points(part.points.iter().copied())?.expand(outline);
        let b = Bounds::from_points(local.corners().into_iter().map(|p| actor.local_to_world(p)))?;
        all = Some(match all {
            None => b,
            Some(a) => Bounds::from_points([a.min, a.max, b.min, b.max])?,
        });
    }
    all
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scene(camera: &str) -> EvaluatedScene {
        let script = motion_script::parse(&format!(
            "project: {{title: f, fps: 24, width: 640, height: 360}}\n\
             characters: {{ALEX: {{asset: a}}, BEA: {{asset: b}}}}\n\
             scenes:\n  - id: s\n    duration: 2\n    composition:\n      camera: [{camera}]\n    events:\n      \
             - {{type: move, actor: ALEX, at: 0, x: 220, y: 250, duration: 1}}\n      \
             - {{type: move, actor: BEA, at: 0, x: 420, y: 250, duration: 1}}\n"
        ))
        .unwrap();
        motion_scene::evaluate(&script, 0, 1.0).unwrap()
    }

    fn two_actors() -> Vec<Subject> {
        vec![Subject::Actor("ALEX".into()), Subject::Actor("BEA".into())]
    }

    #[test]
    fn wide_shot_keeps_both_actors_inside_margin() {
        let wide = scene("{at: 0, x: 0, y: 0, zoom: 1}");
        assert_eq!(
            check_framing(&wide, &two_actors(), FramingRule::WIDE),
            vec![]
        );
        // Panning far right pushes ALEX out of the frame.
        let panned = scene("{at: 0, x: 200, y: 0, zoom: 1}");
        let v = check_framing(&panned, &two_actors(), FramingRule::WIDE);
        assert!(
            matches!(&v[..], [FramingViolation::OutsideMargin { subject: Subject::Actor(id), .. }] if id == "ALEX")
        );
    }

    #[test]
    fn close_up_requires_head_inside_margin_and_large_enough() {
        let wide = scene("{at: 0, x: 0, y: 0, zoom: 1}");
        let head = Subject::Head("ALEX".into());
        let world = subject_world_bounds(&wide, &head).unwrap();
        let center = Vec2::new(
            (world.min.x + world.max.x) / 2.0,
            (world.min.y + world.max.y) / 2.0,
        );
        // Camera offset that puts the head centre at the canvas centre.
        let (dx, dy) = (center.x - 320.0, center.y - 180.0);
        let close = scene(&format!("{{at: 0, x: {dx}, y: {dy}, zoom: 2.4}}"));
        assert_eq!(
            check_framing(&close, std::slice::from_ref(&head), FramingRule::CLOSE_UP),
            vec![]
        );
        // The procedural hand hangs at hip height: outside a face close-up.
        let hand = Subject::Hand("ALEX".into(), Hand::Right);
        assert!(matches!(
            &check_framing(&close, &[head.clone(), hand.clone()], FramingRule::CLOSE_UP)[..],
            [FramingViolation::OutsideMargin { subject, .. }] if *subject == hand
        ));
        assert_eq!(check_framing(&wide, &[hand], FramingRule::WIDE), vec![]);
        // The wide framing is too small for a close-up.
        assert!(matches!(
            &check_framing(&wide, std::slice::from_ref(&head), FramingRule::CLOSE_UP)[..],
            [FramingViolation::TooSmall { .. }]
        ));
        // Too much zoom pushes the head across the 8 % margin.
        let tight = scene(&format!("{{at: 0, x: {dx}, y: {dy}, zoom: 8}}"));
        assert!(matches!(
            &check_framing(&tight, std::slice::from_ref(&head), FramingRule::CLOSE_UP)[..],
            [FramingViolation::OutsideMargin { .. }]
        ));
    }

    #[test]
    fn missing_subjects_are_reported_not_ignored() {
        let wide = scene("{at: 0, x: 0, y: 0, zoom: 1}");
        assert_eq!(
            check_framing(&wide, &[Subject::Head("NOBODY".into())], FramingRule::WIDE),
            vec![FramingViolation::MissingActor(Subject::Head(
                "NOBODY".into()
            ))]
        );
    }

    #[test]
    fn projection_matches_camera_rule() {
        let s = scene("{at: 0, x: 50, y: 20, zoom: 2}");
        let b = project(
            &s,
            Bounds {
                min: Vec2::new(320.0, 180.0),
                max: Vec2::new(330.0, 190.0),
            },
        );
        assert_eq!(b.min, Vec2::new(220.0, 140.0));
        assert_eq!(b.max, Vec2::new(240.0, 160.0));
    }
}
