//! Scene evaluation of `composition.props` (PRO P06): hide the parts the
//! prop's current state hides, and move its parts from the hand they are
//! attached to onto the carrier `motion_rig::PropTrack` names at that time.
use crate::{ActorPose, Error};
use motion_rig::{Affine2, HandSide, PropCarrier};
use motion_script::{ResolvedPropCue, Script};
use std::collections::BTreeMap;

pub(crate) type SceneProps = BTreeMap<String, ResolvedPropCue>;

/// Resolve every scene's prop cues once.
pub(crate) fn prepare(script: &Script) -> Result<Vec<SceneProps>, Error> {
    script
        .scenes
        .iter()
        .map(|scene| Ok(script.resolve_prop_cues(scene)?))
        .collect()
}

/// Actor-local frames of the left and right hand joints for this pose.
pub(crate) fn hand_frames(
    rig: &motion_rig::PreparedRig,
    offsets: &[(usize, motion_rig::Transform2)],
) -> Result<[Affine2; 2], Error> {
    let invalid = |e: String| Error::InvalidAnimation(format!("prop hands: {e}"));
    let world = rig
        .world_transforms(offsets)
        .map_err(|e| invalid(e.to_string()))?;
    let frame = |hand: HandSide| {
        rig.joint_index(hand.joint())
            .and_then(|index| world.get(index).copied())
            .ok_or_else(|| invalid(format!("rig has no {} joint", hand.joint())))
    };
    Ok([frame(HandSide::Left)?, frame(HandSide::Right)?])
}

/// Apply the cue to a finished pose (root transform included).
pub(crate) fn apply(pose: &mut ActorPose, cue: &ResolvedPropCue, hands: [Affine2; 2], time: f64) {
    let start = cue.start();
    let prefix = motion_rig::held_part_id(start, &cue.item.id, "");
    let hidden: Vec<String> = cue
        .hidden_at(time)
        .iter()
        .map(|part| motion_rig::held_part_id(start, &cue.item.id, part))
        .collect();
    pose.rigid_parts.retain(|part| !hidden.contains(&part.id));
    if cue.track.is_static() {
        return;
    }
    let hand = |side: HandSide| match side {
        HandSide::Left => hands[0],
        HandSide::Right => hands[1],
    };
    // Anchors are scene-space; parts are actor-local under the root pose.
    let root = motion_rig::Transform2 {
        translation: motion_rig::Vec2::new(pose.x, pose.y),
        rotation: pose.rotation,
        scale: pose.scale,
    }
    .affine()
    .inverse();
    let target = cue.track.frame(time, |carrier| match carrier {
        PropCarrier::Hand(side) => hand(side),
        PropCarrier::Anchor(index) => root.compose(cue.anchors[index]),
    });
    let carry = target.compose(hand(start).inverse());
    for part in pose
        .rigid_parts
        .iter_mut()
        .filter(|part| part.id.starts_with(&prefix))
    {
        for point in &mut part.points {
            *point = carry.point(*point);
        }
        if let Some(texture) = &mut part.texture {
            texture.transform = carry.compose(texture.transform);
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::ValidatedScript;

    fn veikko_source() -> String {
        let project: serde_json::Value = serde_json::from_str(include_str!(
            "../../../examples/stick-studio/characters/veikko.kilsat"
        ))
        .unwrap();
        project["source"].as_str().unwrap().to_owned()
    }
    /// Veikko with the coffee cup in the right hand and the given prop cues.
    /// The actor walks right during the scene, so hand frames keep changing.
    fn script(composition: &str) -> ValidatedScript {
        let mut value: serde_yaml::Value = serde_yaml::from_str(&veikko_source()).unwrap();
        value["characters"]["VEIKKO"]["holds"] =
            serde_yaml::from_str("{item: kahvikuppi, hand: right}").unwrap();
        let extra: serde_yaml::Mapping = serde_yaml::from_str(composition).unwrap();
        let target = value["scenes"][0]["composition"].as_mapping_mut().unwrap();
        for (key, entry) in extra {
            target.insert(key, entry);
        }
        value["scenes"][0]["events"] = serde_yaml::from_str(
            "[{type: move, at: 0, actor: VEIKKO, x: 520, y: 760, duration: 4}]",
        )
        .unwrap();
        let script = motion_script::parse(&serde_yaml::to_string(&value).unwrap()).unwrap();
        ValidatedScript::new(std::sync::Arc::new(script)).unwrap()
    }
    const CUES: &str = r#"
anchors: {table.top: {x: 300, y: 600}}
props:
  - actor: VEIKKO
    item: kahvikuppi
    actions:
      - {at: 1.0, hand_off: {from: right, to: left}, duration: 0.5}
      - {at: 2.0, prop_state: empty}
      - {at: 2.5, set_down: {anchor: table.top}, duration: 0.8}
"#;
    /// World-space points of the cup's parts at `time`, by part id.
    fn cup(script: &ValidatedScript, time: f64) -> Vec<(String, Vec<motion_rig::Vec2>)> {
        let scene = script.evaluate(0, time).unwrap();
        let actor = scene.actors.iter().find(|a| a.id == "VEIKKO").unwrap();
        actor
            .rigid_parts
            .iter()
            .filter(|p| p.id.starts_with("heldprop_r_kahvikuppi_"))
            .map(|p| {
                (
                    p.id.clone(),
                    p.points.iter().map(|q| actor.local_to_world(*q)).collect(),
                )
            })
            .collect()
    }

    #[test]
    fn scenes_without_cues_are_unchanged_and_static_cues_only_hide() {
        let plain = script("{}");
        let with_cue = script("props: [{actor: VEIKKO, item: kahvikuppi}]");
        for time in [0.0, 1.3, 4.9] {
            assert_eq!(
                plain.evaluate(0, time).unwrap(),
                with_cue.evaluate(0, time).unwrap()
            );
        }
    }

    #[test]
    fn cup_follows_carriers_and_lands_on_the_anchor() {
        let script = script(CUES);
        let before = cup(&script, 0.5);
        assert!(!before.is_empty());
        // On the table the cup's grip point (joint-local origin) sits on the anchor,
        // even while the actor keeps walking.
        let rest_on_table = cup(&script, 3.5);
        let later = cup(&script, 3.9);
        assert_eq!(rest_on_table.len(), later.len());
        for ((_, a), (_, b)) in rest_on_table.iter().zip(&later) {
            for (p, q) in a.iter().zip(b) {
                assert!((p.x - q.x).abs() < 1e-6 && (p.y - q.y).abs() < 1e-6);
            }
        }
        // `empty` hides at least one cup part.
        assert!(cup(&script, 2.1).len() < cup(&script, 1.9).len());
    }

    #[test]
    fn cup_never_jumps_while_hands_and_actor_move() {
        let script = script(CUES);
        // Sample at 2400 Hz across both transfers; with the actor walking, per-sample
        // motion stays tiny wherever the carrier changes. Compare the parts present
        // in both samples (the `empty` state removes some at 2.0 s).
        let step = 1.0 / 2400.0;
        let mut previous = cup(&script, 0.8);
        let mut time = 0.8;
        let mut largest: f64 = 0.0;
        while time < 3.6 {
            time += step;
            let current = cup(&script, time);
            for (id, points) in &current {
                if let Some((_, old)) = previous.iter().find(|(o, _)| o == id) {
                    for (p, q) in points.iter().zip(old) {
                        largest = largest.max((p.x - q.x).hypot(p.y - q.y));
                    }
                }
            }
            previous = current;
        }
        assert!(largest < 1.0, "cup moved {largest} px in one 1/2400 s step");
    }
}
