//! Named prop states and the prop carrier timeline (PRO P06).
//!
//! * A catalog item may list named `states` (for example `full` / `empty`).
//!   The first state is the default and hides nothing; every other state hides
//!   some of the item's parts. An item without `states` has the single
//!   implicit state [`DEFAULT_PROP_STATE`].
//! * A [`PropTrack`] says who carries one prop over scene time: the hand it
//!   starts in, then timed transfers to the other hand (`hand_off`) or to a
//!   scene anchor (`set_down`). During a transfer the prop's frame blends from
//!   the old carrier to the new one with a smoothstep, so the prop's world
//!   position is continuous: it starts exactly on the old carrier and ends
//!   exactly on the new one. Hands do not reach for each other automatically;
//!   animate them with joint tracks or IK when they should meet.
//!
//! Everything here is a pure function of scene time, so preview, seeking and
//! export agree.
use crate::{Affine2, HandPropItem, HandSide, PropError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The state of an item that declares no `states`.
pub const DEFAULT_PROP_STATE: &str = "default";
/// Most states one catalog item may declare.
pub const MAX_PROP_STATES: usize = 8;
/// Most timed transfers on one prop track.
pub const MAX_PROP_TRANSFERS: usize = 64;
/// Longest transfer blend.
pub const MAX_PROP_TRANSFER_SECONDS: f64 = 10.0;
/// Blend length of a `hand_off` without `duration`.
pub const DEFAULT_HAND_OFF_SECONDS: f64 = 0.4;
/// Blend length of a `set_down` without `duration`.
pub const DEFAULT_SET_DOWN_SECONDS: f64 = 0.5;

/// One named appearance of a catalog item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropStateDefinition {
    /// `[a-z0-9_]{1,64}`.
    pub id: String,
    /// Item part ids (from `parts` or `grip_parts`) not drawn in this state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hide: Vec<String>,
}

/// A state name that the item does not declare.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("prop {item} has no state {state}; its states are {known}")]
pub struct UnknownPropState {
    pub item: String,
    pub state: String,
    /// Comma-separated declared states.
    pub known: String,
}

fn state_id_ok(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

impl HandPropItem {
    /// Declared state ids in order; `[DEFAULT_PROP_STATE]` when none are declared.
    pub fn state_ids(&self) -> Vec<&str> {
        if self.states.is_empty() {
            vec![DEFAULT_PROP_STATE]
        } else {
            self.states.iter().map(|s| s.id.as_str()).collect()
        }
    }
    /// The state the item is in until a `prop_state` changes it.
    pub fn default_state(&self) -> &str {
        self.states
            .first()
            .map_or(DEFAULT_PROP_STATE, |s| s.id.as_str())
    }
    /// Item part ids hidden in `state`.
    pub fn hidden_parts(&self, state: &str) -> Result<&[String], UnknownPropState> {
        if self.states.is_empty() && state == DEFAULT_PROP_STATE {
            return Ok(&[]);
        }
        self.states
            .iter()
            .find(|s| s.id == state)
            .map(|s| s.hide.as_slice())
            .ok_or_else(|| UnknownPropState {
                item: self.id.clone(),
                state: state.into(),
                known: self.state_ids().join(", "),
            })
    }
    /// Catalog validation of `states`.
    pub(crate) fn validate_states(&self) -> Result<(), PropError> {
        let fail = |reason: &str| PropError::States {
            item: self.id.clone(),
            reason: reason.into(),
        };
        if self.states.len() > MAX_PROP_STATES {
            return Err(fail("at most 8 states"));
        }
        let parts: BTreeSet<&str> = self
            .parts
            .iter()
            .chain(&self.grip_parts)
            .map(|p| p.id.as_str())
            .collect();
        let mut ids = BTreeSet::new();
        for (index, state) in self.states.iter().enumerate() {
            if !state_id_ok(&state.id) || !ids.insert(state.id.as_str()) {
                return Err(fail("state ids must be unique and match [a-z0-9_]{1,64}"));
            }
            if index == 0 && !state.hide.is_empty() {
                return Err(fail("the first state is the default and must hide nothing"));
            }
            let mut hidden = BTreeSet::new();
            if !state
                .hide
                .iter()
                .all(|p| parts.contains(p.as_str()) && hidden.insert(p))
            {
                return Err(fail("hide must name the item's own parts, each once"));
            }
            if self
                .screen
                .as_ref()
                .is_some_and(|s| hidden.contains(&s.part))
            {
                return Err(fail("a state cannot hide the screen glass"));
            }
        }
        Ok(())
    }
}

/// Who carries a prop: a hand of the actor holding it, or one of the track's
/// anchors (an index into the anchors the caller resolved for this track).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropCarrier {
    Hand(HandSide),
    Anchor(usize),
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Transfer {
    at: f64,
    duration: f64,
    to: PropCarrier,
}

/// Where a prop is at one instant: blending from `from` to `to` by `blend`
/// in `0..=1` (already eased). Outside transfers `from == to`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PropSample {
    pub from: PropCarrier,
    pub to: PropCarrier,
    pub blend: f64,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PropTrackError {
    #[error("time and duration must be finite, at >= 0 and 0 < duration <= {MAX_PROP_TRANSFER_SECONDS} s")]
    Time,
    #[error("at most {MAX_PROP_TRANSFERS} transfers per prop and scene")]
    TooMany,
    #[error("starts at {at} s, before the previous transfer of this prop ends at {busy_until} s")]
    Overlap { at: f64, busy_until: f64 },
    #[error("the prop is already there")]
    SameCarrier,
}

/// Timed carriers of one prop within one scene.
#[derive(Debug, Clone, PartialEq)]
pub struct PropTrack {
    start: HandSide,
    transfers: Vec<Transfer>,
}

impl PropTrack {
    /// A prop held in `start` from the scene's beginning.
    pub fn new(start: HandSide) -> Self {
        Self {
            start,
            transfers: Vec::new(),
        }
    }
    /// The hand the prop starts in.
    pub fn start(&self) -> HandSide {
        self.start
    }
    /// True when the prop never leaves its starting hand.
    pub fn is_static(&self) -> bool {
        self.transfers.is_empty()
    }
    /// The carrier once every transfer started at or before `time` has finished.
    pub fn carrier_after(&self, time: f64) -> PropCarrier {
        self.transfers
            .iter()
            .rev()
            .find(|t| t.at <= time)
            .map_or(PropCarrier::Hand(self.start), |t| t.to)
    }
    /// Carriers the prop occupies at `time`: one, or two while blending.
    pub fn occupies(&self, time: f64) -> [Option<PropCarrier>; 2] {
        let sample = self.sample(time);
        if sample.from == sample.to {
            [Some(sample.to), None]
        } else {
            [Some(sample.from), Some(sample.to)]
        }
    }
    /// When the last transfer ends (0 for a static prop).
    pub fn busy_until(&self) -> f64 {
        self.transfers.last().map_or(0.0, |t| t.at + t.duration)
    }
    /// Append a transfer to `to` starting at `at` and blending over `duration`.
    /// Transfers are appended in time order and may not overlap.
    pub fn push(&mut self, at: f64, duration: f64, to: PropCarrier) -> Result<(), PropTrackError> {
        if !at.is_finite()
            || at < 0.0
            || !duration.is_finite()
            || duration <= 0.0
            || duration > MAX_PROP_TRANSFER_SECONDS
        {
            return Err(PropTrackError::Time);
        }
        if self.transfers.len() >= MAX_PROP_TRANSFERS {
            return Err(PropTrackError::TooMany);
        }
        let busy_until = self.busy_until();
        if !self.transfers.is_empty() && at < busy_until {
            return Err(PropTrackError::Overlap { at, busy_until });
        }
        if self.carrier_after(at) == to {
            return Err(PropTrackError::SameCarrier);
        }
        self.transfers.push(Transfer { at, duration, to });
        Ok(())
    }
    /// Carriers and eased blend at `time`.
    pub fn sample(&self, time: f64) -> PropSample {
        let mut from = PropCarrier::Hand(self.start);
        for transfer in &self.transfers {
            if transfer.at > time {
                break;
            }
            let progress = (time - transfer.at) / transfer.duration;
            if progress < 1.0 {
                return PropSample {
                    from,
                    to: transfer.to,
                    blend: smoothstep(progress),
                };
            }
            from = transfer.to;
        }
        PropSample {
            from,
            to: from,
            blend: 0.0,
        }
    }
    /// The prop's grip frame at `time`, given each carrier's current frame.
    pub fn frame(&self, time: f64, carrier_frame: impl Fn(PropCarrier) -> Affine2) -> Affine2 {
        let sample = self.sample(time);
        if sample.from == sample.to {
            return carrier_frame(sample.to);
        }
        blend_frames(
            carrier_frame(sample.from),
            carrier_frame(sample.to),
            sample.blend,
        )
    }
}

/// `3w² - 2w³` on `w` clamped to `0..=1`.
pub fn smoothstep(w: f64) -> f64 {
    let w = w.clamp(0.0, 1.0);
    w * w * (3.0 - 2.0 * w)
}

/// Blend two frames: the origin moves on a straight line, rotation takes the
/// shorter way and scale interpolates linearly. Exact at `w = 0` and `w = 1`.
/// Shear in the relative transform is dropped while blending.
pub fn blend_frames(from: Affine2, to: Affine2, w: f64) -> Affine2 {
    if w <= 0.0 {
        return from;
    }
    if w >= 1.0 {
        return to;
    }
    let delta = from.inverse().compose(to);
    let angle = delta.b.atan2(delta.a);
    let sx = delta.a.hypot(delta.b);
    let sy = if sx > 0.0 {
        (delta.a * delta.d - delta.b * delta.c) / sx
    } else {
        1.0
    };
    let step = crate::Transform2 {
        translation: crate::Vec2::new(delta.tx * w, delta.ty * w),
        rotation: angle * w,
        scale: crate::Vec2::new(1.0 + (sx - 1.0) * w, 1.0 + (sy - 1.0) * w),
    };
    // `Transform2::affine` composes T·R·S, matching the decomposition above.
    let (s, c) = step.rotation.sin_cos();
    from.compose(Affine2 {
        a: c * step.scale.x,
        b: s * step.scale.x,
        c: -s * step.scale.y,
        d: c * step.scale.y,
        tx: step.translation.x,
        ty: step.translation.y,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{HandPropCatalog, Vec2};

    const CATALOG: &str = include_str!("../../../examples/stick-studio/hand-props.json");

    fn frame(x: f64, y: f64, rotation: f64, scale: f64) -> Affine2 {
        crate::Transform2 {
            translation: Vec2::new(x, y),
            rotation,
            scale: Vec2::new(scale, scale),
        }
        .affine()
    }
    fn close(a: Affine2, b: Affine2) -> bool {
        [
            a.a - b.a,
            a.b - b.b,
            a.c - b.c,
            a.d - b.d,
            a.tx - b.tx,
            a.ty - b.ty,
        ]
        .iter()
        .all(|d| d.abs() < 1e-9)
    }

    #[test]
    fn catalog_states_default_first_and_unknown_state_lists_known() {
        let catalog = HandPropCatalog::decode(CATALOG).unwrap();
        let cup = catalog.items.iter().find(|i| i.id == "kahvikuppi").unwrap();
        assert_eq!(cup.state_ids(), ["full", "empty"]);
        assert_eq!(cup.default_state(), "full");
        assert!(cup.hidden_parts("full").unwrap().is_empty());
        assert_eq!(cup.hidden_parts("empty").unwrap(), ["coffee"]);
        let error = cup.hidden_parts("broken").unwrap_err();
        assert_eq!(
            error.to_string(),
            "prop kahvikuppi has no state broken; its states are full, empty"
        );
        let key = catalog.items.iter().find(|i| i.id == "avain").unwrap();
        assert_eq!(key.state_ids(), [DEFAULT_PROP_STATE]);
        assert!(key.hidden_parts(DEFAULT_PROP_STATE).unwrap().is_empty());
        assert!(key.hidden_parts("open").is_err());
    }

    #[test]
    fn invalid_states_are_rejected_by_catalog_validation() {
        let base = HandPropCatalog::decode(CATALOG).unwrap();
        let index = base
            .items
            .iter()
            .position(|i| i.id == "kahvikuppi")
            .unwrap();
        let reject = |edit: &dyn Fn(&mut HandPropItem)| {
            let mut catalog = base.clone();
            edit(&mut catalog.items[index]);
            assert!(
                matches!(catalog.validate(), Err(PropError::States { ref item, .. }) if item == "kahvikuppi"),
                "{:?}",
                catalog.validate()
            );
        };
        reject(&|i| i.states[0].hide.push("coffee".into()));
        reject(&|i| i.states[1].hide.push("nope".into()));
        reject(&|i| i.states[1].hide.push("coffee".into()));
        reject(&|i| i.states[1].id = "full".into());
        reject(&|i| i.states[1].id = "Empty".into());
        reject(&|i| {
            i.states = (0..9)
                .map(|n| PropStateDefinition {
                    id: format!("s{n}"),
                    hide: vec![],
                })
                .collect()
        });
        let mut phone = base.clone();
        let p = phone.items.iter().position(|i| i.id == "puhelin").unwrap();
        phone.items[p].states = vec![
            PropStateDefinition {
                id: "on".into(),
                hide: vec![],
            },
            PropStateDefinition {
                id: "off".into(),
                hide: vec!["screen".into()],
            },
        ];
        assert!(matches!(phone.validate(), Err(PropError::States { .. })));
    }

    #[test]
    fn blend_is_exact_at_the_ends_and_takes_the_short_rotation() {
        let a = frame(10.0, 20.0, 3.0, 1.0);
        let b = frame(-30.0, 5.0, -3.0, 2.0);
        assert!(close(blend_frames(a, b, 0.0), a));
        assert!(close(blend_frames(a, b, 1.0), b));
        assert!(close(blend_frames(a, b, 1.0 - 1e-12), b));
        let half = blend_frames(a, b, 0.5);
        // Origin halfway on the straight line.
        assert!((half.tx + 10.0).abs() < 1e-9 && (half.ty - 12.5).abs() < 1e-9);
        // 3 rad to -3 rad is 0.283 rad the short way, through ±π.
        let angle = half.b.atan2(half.a);
        assert!((angle.abs() - std::f64::consts::PI).abs() < 1e-9, "{angle}");
        assert!((half.a.hypot(half.b) - 1.5).abs() < 1e-9);
    }

    #[test]
    fn track_samples_hand_off_then_set_down_and_rejects_bad_transfers() {
        let mut track = PropTrack::new(HandSide::Left);
        assert!(track.is_static());
        track
            .push(2.0, 0.5, PropCarrier::Hand(HandSide::Right))
            .unwrap();
        track.push(4.0, 1.0, PropCarrier::Anchor(0)).unwrap();
        let left = PropCarrier::Hand(HandSide::Left);
        let right = PropCarrier::Hand(HandSide::Right);
        assert_eq!(
            track.sample(1.0),
            PropSample {
                from: left,
                to: left,
                blend: 0.0
            }
        );
        assert_eq!(track.sample(2.25).blend, 0.5);
        assert_eq!(track.sample(2.25).to, right);
        assert_eq!(track.occupies(2.25), [Some(left), Some(right)]);
        assert_eq!(track.occupies(3.0), [Some(right), None]);
        assert_eq!(track.sample(4.5).from, right);
        assert_eq!(track.carrier_after(9.0), PropCarrier::Anchor(0));
        assert_eq!(track.busy_until(), 5.0);
        assert_eq!(
            track.push(4.5, 0.5, left),
            Err(PropTrackError::Overlap {
                at: 4.5,
                busy_until: 5.0
            })
        );
        assert_eq!(
            track.push(6.0, 0.5, PropCarrier::Anchor(0)),
            Err(PropTrackError::SameCarrier)
        );
        for (at, duration) in [(f64::NAN, 0.5), (6.0, 0.0), (6.0, 11.0), (-1.0, 0.5)] {
            assert_eq!(track.push(at, duration, left), Err(PropTrackError::Time));
        }
    }

    /// The prop's world position never jumps while hands move and the prop
    /// changes hands and is set down: per-step motion stays within what the
    /// carriers' own motion and the blend speed allow.
    #[test]
    fn prop_world_position_is_continuous_through_hand_off_and_set_down() {
        let mut track = PropTrack::new(HandSide::Left);
        track
            .push(2.0, 0.4, PropCarrier::Hand(HandSide::Right))
            .unwrap();
        track.push(3.5, 0.5, PropCarrier::Anchor(0)).unwrap();
        // Hands move along sinusoids 60 px apart; the anchor is a table top.
        let carrier = |t: f64| {
            move |c: PropCarrier| match c {
                PropCarrier::Hand(HandSide::Left) => {
                    frame(-30.0 + 5.0 * t.sin(), -100.0, 0.4 * t.cos(), 1.0)
                }
                PropCarrier::Hand(HandSide::Right) => {
                    frame(30.0, -100.0 + 4.0 * t.cos(), -0.3, 1.0)
                }
                PropCarrier::Anchor(_) => frame(200.0, -60.0, 0.0, 1.0),
            }
        };
        let dt = 1.0 / 2400.0;
        let mut previous = track.frame(0.0, carrier(0.0));
        let mut largest: f64 = 0.0;
        let mut t = dt;
        while t <= 5.0 {
            let now = track.frame(t, carrier(t));
            let step = (now.tx - previous.tx).hypot(now.ty - previous.ty);
            largest = largest.max(step);
            previous = now;
            t += dt;
        }
        // Fastest blend: ~250 px over 0.5 s with smoothstep peak speed 1.5x,
        // so about 0.32 px per 1/2400 s step. A jump would be tens of pixels.
        assert!(largest < 0.5, "largest step {largest}");
        // And it ends exactly on the anchor.
        assert!(close(
            track.frame(5.0, carrier(5.0)),
            frame(200.0, -60.0, 0.0, 1.0)
        ));
    }
}
