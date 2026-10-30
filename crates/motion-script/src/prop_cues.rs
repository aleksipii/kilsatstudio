//! `composition.props`: timed hand-offs, set-downs and state changes of a
//! character's `holds:` prop (PRO P06).
//!
//! ```yaml
//! composition:
//!   anchors:
//!     table.top: {x: 820, y: 610}
//!   props:
//!     - actor: VEIKKO
//!       item: kahvikuppi
//!       actions:
//!         - {at: 2.0, hand_off: {from: right, to: left}}
//!         - {at: 4.0, set_down: {anchor: table.top}, duration: 0.6}
//!         - {at: 5.0, prop_state: empty}
//! ```
//!
//! The prop starts in the hand named by the character's `holds:`. Anchors are
//! scene-space points (pixels, like gaze targets) with an optional rotation;
//! a set-down puts the item's grip point on the anchor. Transfers blend the
//! prop between carriers (see `motion_rig::PropTrack`), so it never jumps.
//! `prop_state` switches the item's named state instantly.
//!
//! These cues live in `composition` rather than `events`, so event consumers
//! are unaffected.
use crate::{Scene, Script};
use motion_rig::{
    Affine2, HandPropItem, HandSide, PropCarrier, PropTrack, PropTrackError, UnknownPropState,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Most anchors per scene.
pub const MAX_PROP_ANCHORS: usize = 64;
/// Most prop cues per scene.
pub const MAX_PROP_CUES: usize = 32;
/// Most actions on one prop cue.
pub const MAX_PROP_ACTIONS: usize = 128;
/// Largest anchor coordinate magnitude, in scene pixels.
pub const MAX_PROP_ANCHOR_COORD: f64 = 100_000.0;

/// A named scene point a prop can be set down on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropAnchor {
    pub x: f64,
    pub y: f64,
    /// Rotation of the set-down prop, in degrees (positive is clockwise on screen).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub rotation_degrees: f64,
}
fn is_zero(value: &f64) -> bool {
    *value == 0.0
}
impl PropAnchor {
    /// The anchor as a scene-space frame.
    pub fn frame(&self) -> Affine2 {
        motion_rig::Transform2 {
            translation: motion_rig::Vec2::new(self.x, self.y),
            rotation: self.rotation_degrees.to_radians(),
            scale: motion_rig::Vec2::new(1.0, 1.0),
        }
        .affine()
    }
}

/// Timed actions on one actor's held prop.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropCue {
    pub actor: String,
    /// The catalog item the actor `holds:`.
    pub item: String,
    #[serde(default)]
    pub actions: Vec<PropAction>,
}

/// One timed action: exactly one of `hand_off`, `set_down` or `prop_state`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropAction {
    pub at: f64,
    /// Blend length for `hand_off` / `set_down`; defaults to
    /// `motion_rig::DEFAULT_HAND_OFF_SECONDS` / `DEFAULT_SET_DOWN_SECONDS`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hand_off: Option<HandOff>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_down: Option<SetDown>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prop_state: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandOff {
    pub from: HandSide,
    pub to: HandSide,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetDown {
    pub anchor: String,
}

/// Why a prop cue is invalid. Each message ends with how to fix it.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum PropCueError {
    #[error("at most {MAX_PROP_ANCHORS} anchors and {MAX_PROP_CUES} prop cues per scene; split the scene")]
    TooMany,
    #[error("anchor {anchor}: names are dot-separated [a-z0-9_] words up to 64 bytes, x and y finite within ±{MAX_PROP_ANCHOR_COORD} and rotation_degrees within ±360")]
    InvalidAnchor { anchor: String },
    #[error("unknown actor {actor}; characters are {known}")]
    UnknownActor { actor: String, known: String },
    #[error("{actor} has more than one prop cue; put all actions for its prop in one cue")]
    Duplicate { actor: String },
    #[error("{actor} holds nothing; add `holds: {{item: {item}, hand: right}}` to the character")]
    NotHeld { actor: String, item: String },
    #[error("{actor} holds {held}, not {item}; name {held} or change the character's `holds:`")]
    WrongItem {
        actor: String,
        item: String,
        held: String,
    },
    #[error("unknown item {item}; the stick-studio hand prop catalog has {known}")]
    UnknownItem { item: String, known: String },
    #[error("{actor} also has timed `hold` events in this scene; use either `hold` events or prop cues for an actor in one scene")]
    TimedHold { actor: String },
    #[error("at most {MAX_PROP_ACTIONS} actions per prop cue")]
    TooManyActions,
    #[error("action at {at} s: give exactly one of hand_off, set_down or prop_state")]
    ActionKind { at: f64 },
    #[error("action at {at} s: `at` must be finite, within the scene (0..={scene_duration} s) and not before the previous action")]
    Time { at: f64, scene_duration: f64 },
    #[error("action at {at} s: prop_state switches instantly; remove `duration`")]
    StateDuration { at: f64 },
    #[error("action at {at} s: unknown anchor {anchor}; declare it under composition.anchors (known: {known})")]
    UnknownAnchor {
        at: f64,
        anchor: String,
        known: String,
    },
    #[error("action at {at} s: hand_off from the {from:?} hand, but the prop is {actual} then; hand off from where it is")]
    WrongHand {
        at: f64,
        from: HandSide,
        actual: String,
    },
    #[error("action at {at} s: {source}; use a state the item declares")]
    UnknownState { at: f64, source: UnknownPropState },
    #[error("action at {at} s: {source}")]
    Track { at: f64, source: PropTrackError },
    #[error("action at {at} s: the transfer ends after the scene ({scene_duration} s); start it earlier or shorten `duration`")]
    PastScene { at: f64, scene_duration: f64 },
}

/// A validated prop cue ready for evaluation.
#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedPropCue {
    pub item: HandPropItem,
    pub track: PropTrack,
    /// Scene-space frames of the anchors the track names, by `PropCarrier::Anchor` index.
    pub anchors: Vec<Affine2>,
    /// `(at, state)` changes in time order, after the item's default state.
    states: Vec<(f64, String)>,
}
impl ResolvedPropCue {
    /// The hand the prop starts in (its parts are attached there).
    pub fn start(&self) -> HandSide {
        self.track.start()
    }
    /// The item's state at scene time `time`.
    pub fn state_at(&self, time: f64) -> &str {
        self.states
            .iter()
            .rev()
            .find(|(at, _)| *at <= time)
            .map_or(self.item.default_state(), |(_, s)| s.as_str())
    }
    /// Item part ids hidden at scene time `time`.
    pub fn hidden_at(&self, time: f64) -> &[String] {
        // States were checked at resolve time.
        self.item.hidden_parts(self.state_at(time)).unwrap_or(&[])
    }
}

fn anchor_name_ok(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.split('.').all(|word| {
            !word.is_empty()
                && word
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        })
}

fn catalog_item(item: &str) -> Result<HandPropItem, PropCueError> {
    let catalog = motion_rig::stick_studio_hand_props().map_err(|e| PropCueError::UnknownItem {
        item: item.into(),
        known: e.to_string(),
    })?;
    catalog
        .items
        .iter()
        .find(|i| i.id == item)
        .cloned()
        .ok_or_else(|| PropCueError::UnknownItem {
            item: item.into(),
            known: catalog
                .items
                .iter()
                .map(|i| i.id.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        })
}

fn carrier_name(carrier: PropCarrier, anchors: &[&str]) -> String {
    match carrier {
        PropCarrier::Hand(hand) => format!("in the {hand:?} hand"),
        PropCarrier::Anchor(index) => format!("on {}", anchors[index]),
    }
}

impl PropCue {
    /// Check this cue against the script and its scene and build its track.
    pub fn resolve(&self, script: &Script, scene: &Scene) -> Result<ResolvedPropCue, PropCueError> {
        let character =
            script
                .characters
                .get(&self.actor)
                .ok_or_else(|| PropCueError::UnknownActor {
                    actor: self.actor.clone(),
                    known: script
                        .characters
                        .keys()
                        .map(String::as_str)
                        .collect::<Vec<_>>()
                        .join(", "),
                })?;
        let item = catalog_item(&self.item)?;
        let held = character
            .holds
            .as_ref()
            .ok_or_else(|| PropCueError::NotHeld {
                actor: self.actor.clone(),
                item: self.item.clone(),
            })?;
        if held.item != self.item {
            return Err(PropCueError::WrongItem {
                actor: self.actor.clone(),
                item: self.item.clone(),
                held: held.item.clone(),
            });
        }
        if scene
            .events
            .iter()
            .any(|e| matches!(e, crate::Event::Hold { actor, .. } if *actor == self.actor))
        {
            return Err(PropCueError::TimedHold {
                actor: self.actor.clone(),
            });
        }
        if self.actions.len() > MAX_PROP_ACTIONS {
            return Err(PropCueError::TooManyActions);
        }
        let known_anchors = || {
            let names: Vec<&str> = scene
                .composition
                .anchors
                .keys()
                .map(String::as_str)
                .collect();
            if names.is_empty() {
                "none".to_owned()
            } else {
                names.join(", ")
            }
        };
        let mut track = PropTrack::new(held.hand);
        let mut anchor_names: Vec<&str> = Vec::new();
        let mut anchors = Vec::new();
        let mut states = Vec::new();
        let mut previous = 0.0;
        for action in &self.actions {
            let at = action.at;
            if !at.is_finite() || at < previous || at > scene.duration {
                return Err(PropCueError::Time {
                    at,
                    scene_duration: scene.duration,
                });
            }
            previous = at;
            let kinds = [
                action.hand_off.is_some(),
                action.set_down.is_some(),
                action.prop_state.is_some(),
            ];
            if kinds.iter().filter(|k| **k).count() != 1 {
                return Err(PropCueError::ActionKind { at });
            }
            if let Some(state) = &action.prop_state {
                if action.duration.is_some() {
                    return Err(PropCueError::StateDuration { at });
                }
                item.hidden_parts(state)
                    .map_err(|source| PropCueError::UnknownState { at, source })?;
                states.push((at, state.clone()));
                continue;
            }
            let (to, default) = if let Some(hand_off) = action.hand_off {
                let actual = track.carrier_after(at);
                if actual != PropCarrier::Hand(hand_off.from) {
                    return Err(PropCueError::WrongHand {
                        at,
                        from: hand_off.from,
                        actual: carrier_name(actual, &anchor_names),
                    });
                }
                (
                    PropCarrier::Hand(hand_off.to),
                    motion_rig::DEFAULT_HAND_OFF_SECONDS,
                )
            } else {
                let name = &action.set_down.as_ref().expect("one kind").anchor;
                let anchor = scene.composition.anchors.get(name).ok_or_else(|| {
                    PropCueError::UnknownAnchor {
                        at,
                        anchor: name.clone(),
                        known: known_anchors(),
                    }
                })?;
                let index = match anchor_names.iter().position(|n| *n == name.as_str()) {
                    Some(index) => index,
                    None => {
                        anchor_names.push(name.as_str());
                        anchors.push(anchor.frame());
                        anchor_names.len() - 1
                    }
                };
                (
                    PropCarrier::Anchor(index),
                    motion_rig::DEFAULT_SET_DOWN_SECONDS,
                )
            };
            let duration = action.duration.unwrap_or(default);
            track
                .push(at, duration, to)
                .map_err(|source| PropCueError::Track { at, source })?;
            if at + duration > scene.duration {
                return Err(PropCueError::PastScene {
                    at,
                    scene_duration: scene.duration,
                });
            }
        }
        Ok(ResolvedPropCue {
            item,
            track,
            anchors,
            states,
        })
    }
}

impl Script {
    /// Resolved prop cues of scene `scene` by actor. Fails like validation.
    pub fn resolve_prop_cues(
        &self,
        scene: &Scene,
    ) -> Result<BTreeMap<String, ResolvedPropCue>, crate::Error> {
        let fail = |e: PropCueError| crate::Error::Validation(format!("{}: props: {e}", scene.id));
        let composition = &scene.composition;
        if composition.anchors.len() > MAX_PROP_ANCHORS || composition.props.len() > MAX_PROP_CUES {
            return Err(fail(PropCueError::TooMany));
        }
        for (name, anchor) in &composition.anchors {
            let coord_ok = |v: f64| v.is_finite() && v.abs() <= MAX_PROP_ANCHOR_COORD;
            if !anchor_name_ok(name)
                || !coord_ok(anchor.x)
                || !coord_ok(anchor.y)
                || !anchor.rotation_degrees.is_finite()
                || anchor.rotation_degrees.abs() > 360.0
            {
                return Err(fail(PropCueError::InvalidAnchor {
                    anchor: name.clone(),
                }));
            }
        }
        let mut resolved = BTreeMap::new();
        for cue in &composition.props {
            let prop = cue.resolve(self, scene).map_err(|e| {
                crate::Error::Validation(format!("{}: props for {}: {e}", scene.id, cue.actor))
            })?;
            if resolved.insert(cue.actor.clone(), prop).is_some() {
                return Err(fail(PropCueError::Duplicate {
                    actor: cue.actor.clone(),
                }));
            }
        }
        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn veikko_source() -> String {
        let project: serde_json::Value = serde_json::from_str(include_str!(
            "../../../examples/stick-studio/characters/veikko.kilsat"
        ))
        .unwrap();
        project["source"].as_str().unwrap().to_owned()
    }
    /// Veikko holding the coffee cup in the right hand, with `composition` merged in.
    fn script_with(composition: &str) -> String {
        let mut value: serde_yaml::Value = serde_yaml::from_str(&veikko_source()).unwrap();
        value["characters"]["VEIKKO"]["holds"] =
            serde_yaml::from_str("{item: kahvikuppi, hand: right}").unwrap();
        let extra: serde_yaml::Mapping = serde_yaml::from_str(composition).unwrap();
        let target = value["scenes"][0]["composition"].as_mapping_mut().unwrap();
        for (key, entry) in extra {
            target.insert(key, entry);
        }
        serde_yaml::to_string(&value).unwrap()
    }
    const FULL: &str = r#"
anchors: {table.top: {x: 820, y: 610, rotation_degrees: 5}}
props:
  - actor: VEIKKO
    item: kahvikuppi
    actions:
      - {at: 0.5, hand_off: {from: right, to: left}}
      - {at: 1.0, prop_state: empty}
      - {at: 1.2, set_down: {anchor: table.top}, duration: 0.6}
"#;
    fn error(composition: &str) -> String {
        crate::parse(&script_with(composition))
            .unwrap_err()
            .to_string()
    }

    #[test]
    fn prop_cues_parse_resolve_and_round_trip() {
        let script = crate::parse(&script_with(FULL)).unwrap();
        let scene = &script.scenes[0];
        let cues = script.resolve_prop_cues(scene).unwrap();
        let cue = &cues["VEIKKO"];
        assert_eq!(cue.start(), HandSide::Right);
        assert_eq!(cue.state_at(0.9), "full");
        assert_eq!(cue.state_at(1.0), "empty");
        assert!(cue.hidden_at(0.0).is_empty());
        assert!(!cue.hidden_at(2.0).is_empty());
        assert_eq!(
            cue.track.carrier_after(0.6),
            PropCarrier::Hand(HandSide::Left)
        );
        assert_eq!(cue.track.carrier_after(2.0), PropCarrier::Anchor(0));
        assert_eq!(cue.anchors.len(), 1);
        let text = crate::to_source(&script).unwrap();
        let again = crate::parse(&text).unwrap();
        assert_eq!(again.scenes[0].composition, scene.composition);
    }

    #[test]
    fn scripts_without_prop_cues_serialize_unchanged() {
        let script = crate::parse(&veikko_source()).unwrap();
        let text = crate::to_source(&script).unwrap();
        assert!(
            !text.contains("anchors") && !text.contains("props:"),
            "{text}"
        );
    }

    #[test]
    fn unknown_references_are_typed_and_actionable() {
        let unknown_anchor = error(
            "props: [{actor: VEIKKO, item: kahvikuppi, actions: [{at: 1, set_down: {anchor: shelf.top}}]}]",
        );
        assert!(
            unknown_anchor.contains("unknown anchor shelf.top")
                && unknown_anchor.contains("declare it under composition.anchors")
                && unknown_anchor.contains("known: none"),
            "{unknown_anchor}"
        );
        let unknown_state = error(
            "props: [{actor: VEIKKO, item: kahvikuppi, actions: [{at: 1, prop_state: spilled}]}]",
        );
        assert!(
            unknown_state.contains("no state spilled") && unknown_state.contains("full, empty"),
            "{unknown_state}"
        );
        let unknown_item = error("props: [{actor: VEIKKO, item: kirves}]");
        assert!(
            unknown_item.contains("unknown item kirves") && unknown_item.contains("puhelin"),
            "{unknown_item}"
        );
        let wrong_item = error("props: [{actor: VEIKKO, item: puhelin}]");
        assert!(
            wrong_item.contains("holds kahvikuppi, not puhelin"),
            "{wrong_item}"
        );
        let unknown_actor = error("props: [{actor: SISU, item: kahvikuppi}]");
        assert!(
            unknown_actor.contains("unknown actor SISU") && unknown_actor.contains("VEIKKO"),
            "{unknown_actor}"
        );
        // Typed error values for callers that match on them.
        let script = crate::parse(&script_with("{}")).unwrap();
        let mut scene = script.scenes[0].clone();
        scene.composition.props = vec![PropCue {
            actor: "VEIKKO".into(),
            item: "kahvikuppi".into(),
            actions: vec![PropAction {
                at: 1.0,
                duration: None,
                hand_off: None,
                set_down: Some(SetDown {
                    anchor: "table.top".into(),
                }),
                prop_state: None,
            }],
        }];
        assert!(matches!(
            scene.composition.props[0].resolve(&script, &scene),
            Err(PropCueError::UnknownAnchor { .. })
        ));
    }

    #[test]
    fn timing_and_carrier_errors() {
        let cue = |actions: &str| {
            error(&format!(
                "anchors: {{table.top: {{x: 1, y: 2}}}}\nprops: [{{actor: VEIKKO, item: kahvikuppi, actions: {actions}}}]"
            ))
        };
        assert!(cue("[{at: 1, hand_off: {from: left, to: right}}]")
            .contains("but the prop is in the Right hand"));
        assert!(cue("[{at: 1, hand_off: {from: right, to: right}}]").contains("already there"));
        assert!(cue("[{at: 1, hand_off: {from: right, to: left}}, {at: 1.1, hand_off: {from: left, to: right}}]")
            .contains("before the previous transfer"));
        assert!(cue("[{at: 2}, {at: 1, prop_state: empty}]").contains("exactly one"));
        assert!(
            cue("[{at: 2, prop_state: empty}, {at: 1, prop_state: full}]")
                .contains("not before the previous action")
        );
        assert!(cue("[{at: 1, prop_state: empty, duration: 1}]").contains("remove `duration`"));
        assert!(
            cue("[{at: 1, prop_state: empty, hand_off: {from: right, to: left}}]")
                .contains("exactly one")
        );
        let past = cue("[{at: 9999, set_down: {anchor: table.top}}]");
        assert!(past.contains("within the scene"), "{past}");
        assert!(error("anchors: {Table: {x: 1, y: 2}}").contains("anchor Table"));
        assert!(error("anchors: {table.top: {x: .inf, y: 2}}").contains("anchor table.top"));
        // Unknown fields are schema errors.
        assert!(matches!(
            crate::parse(&script_with(
                "props: [{actor: VEIKKO, item: kahvikuppi, grip: 1}]"
            )),
            Err(crate::Error::Syntax(_))
        ));
    }

    #[test]
    fn prop_cues_need_holds_and_exclude_timed_holds() {
        let mut value: serde_yaml::Value =
            serde_yaml::from_str(&script_with("props: [{actor: VEIKKO, item: kahvikuppi}]"))
                .unwrap();
        value["characters"]["VEIKKO"]
            .as_mapping_mut()
            .unwrap()
            .remove("holds");
        let message = crate::parse(&serde_yaml::to_string(&value).unwrap())
            .unwrap_err()
            .to_string();
        assert!(message.contains("add `holds:"), "{message}");

        let mut value: serde_yaml::Value =
            serde_yaml::from_str(&script_with("props: [{actor: VEIKKO, item: kahvikuppi}]"))
                .unwrap();
        let events = value["scenes"][0]["events"].as_sequence_mut().unwrap();
        events.push(
            serde_yaml::from_str("{type: hold, at: 0.5, actor: VEIKKO, hand: left, item: puhelin}")
                .unwrap(),
        );
        let message = crate::parse(&serde_yaml::to_string(&value).unwrap())
            .unwrap_err()
            .to_string();
        assert!(message.contains("timed `hold` events"), "{message}");
    }
}
