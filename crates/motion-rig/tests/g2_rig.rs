//! G2 rig contract tests against the shipped stick characters (KS-003a):
//! limbs stay attached in every authored clip, two-bone IK reaches its target,
//! and a catalog phone stays in the hand through IK, a walk and a camera change.
use motion_rig::attachment::origin;
use motion_rig::{
    attach_hand_prop, BendDirection, GripPlacement, HandPropCatalog, HandSide, PreparedRig,
    RigDefinition, Transform2, Vec2, STICK_LIMBS,
};
use serde_json::Value;
use std::path::{Path, PathBuf};

fn examples() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn read_json(path: &Path) -> Value {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{path:?}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path:?}: {e}"))
}

/// The MotionScript document embedded in a `.kilsat` project.
fn script(path: &Path) -> Value {
    let project = read_json(path);
    serde_json::from_str(project["source"].as_str().expect("source string")).expect("script")
}

fn rig_of(script: &Value, character: &str) -> RigDefinition {
    serde_json::from_value(script["characters"][character]["rig"].clone())
        .unwrap_or_else(|e| panic!("{character}: {e}"))
}

const CHARACTER_FILES: [(&str, &str); 6] = [
    ("noutaja", "NOUTAJA"),
    ("veikko", "VEIKKO"),
    ("pasma", "PASMA"),
    ("kipina", "KIPINA"),
    ("kiire", "KIIRE"),
    ("sisu", "SISU"),
];

fn stick_rigs() -> Vec<(String, RigDefinition)> {
    CHARACTER_FILES
        .iter()
        .map(|(file, id)| {
            let path = examples().join(format!("stick-studio/characters/{file}.kilsat"));
            (id.to_string(), rig_of(&script(&path), id))
        })
        .collect()
}

/// Every joint-track set (rig clips and per-scene actor tracks) in a script.
fn joint_track_sets(script: &Value) -> Vec<(String, Value)> {
    let mut sets = Vec::new();
    if let Some(clips) = script["rig_clips"].as_object() {
        for (name, clip) in clips {
            sets.push((format!("clip {name}"), clip["joint_tracks"].clone()));
        }
    }
    for scene in script["scenes"].as_array().into_iter().flatten() {
        if let Some(actors) = scene["composition"]["joint_tracks"].as_object() {
            for (actor, tracks) in actors {
                sets.push((format!("{} {actor}", scene["id"]), tracks.clone()));
            }
        }
    }
    sets
}

fn key_offset(key: &Value) -> Transform2 {
    let get = |name: &str, default: f64| key[name].as_f64().unwrap_or(default);
    Transform2 {
        translation: Vec2::new(get("x", 0.0), get("y", 0.0)),
        rotation: get("rotation", 0.0),
        scale: Vec2::new(get("scale_x", 1.0), get("scale_y", 1.0)),
    }
}

/// Smoothstep-interpolated offset of one track at `time` (held outside the keys).
fn sample(keys: &[Value], time: f64) -> Transform2 {
    let at = |k: &Value| k["at"].as_f64().expect("key time");
    let first = &keys[0];
    if time <= at(first) {
        return key_offset(first);
    }
    for pair in keys.windows(2) {
        let (a, b) = (at(&pair[0]), at(&pair[1]));
        if time <= b {
            let u = if b > a { (time - a) / (b - a) } else { 1.0 };
            let s = u * u * (3.0 - 2.0 * u);
            let (p, q) = (key_offset(&pair[0]), key_offset(&pair[1]));
            let lerp = |x: f64, y: f64| x + (y - x) * s;
            return Transform2 {
                translation: Vec2::new(
                    lerp(p.translation.x, q.translation.x),
                    lerp(p.translation.y, q.translation.y),
                ),
                rotation: lerp(p.rotation, q.rotation),
                scale: Vec2::new(lerp(p.scale.x, q.scale.x), lerp(p.scale.y, q.scale.y)),
            };
        }
    }
    key_offset(keys.last().expect("non-empty track"))
}

/// Offsets for every time a track has a key, plus the midpoints between keys.
fn clip_poses(rig: &PreparedRig, tracks: &Value) -> Vec<Vec<(usize, Transform2)>> {
    let Some(tracks) = tracks.as_object() else {
        return Vec::new();
    };
    let mut times: Vec<f64> = tracks
        .values()
        .flat_map(|keys| keys.as_array().into_iter().flatten())
        .filter_map(|k| k["at"].as_f64())
        .collect();
    times.sort_by(f64::total_cmp);
    let midpoints: Vec<f64> = times.windows(2).map(|w| 0.5 * (w[0] + w[1])).collect();
    times.extend(midpoints);
    times
        .iter()
        .map(|&time| {
            tracks
                .iter()
                .filter_map(|(joint, keys)| {
                    let index = rig.joint_index(joint)?;
                    Some((index, sample(keys.as_array()?, time)))
                })
                .collect()
        })
        .collect()
}

#[test]
fn every_stick_character_follows_the_attachment_contract() {
    for (id, rig) in stick_rigs() {
        let prepared = rig.prepare().unwrap_or_else(|e| panic!("{id}: {e}"));
        let limbs = prepared
            .stick_limbs()
            .unwrap_or_else(|e| panic!("{id}: {e}"));
        // Arms hang from the torso (shoulders), legs from the pelvis root.
        let torso = prepared.joint_index("torso").unwrap();
        let pelvis = prepared.joint_index("root").unwrap();
        assert_eq!([limbs[0].anchor, limbs[1].anchor], [torso, torso], "{id}");
        assert_eq!([limbs[2].anchor, limbs[3].anchor], [pelvis, pelvis], "{id}");
    }
}

#[test]
fn limbs_stay_attached_in_every_authored_clip() {
    let mut scripts = vec![script(&examples().join("stick-studio/stick-studio.kilsat"))];
    for (file, _) in CHARACTER_FILES {
        scripts.push(script(
            &examples().join(format!("stick-studio/characters/{file}.kilsat")),
        ));
    }
    let rigs = stick_rigs();
    let mut checked = 0usize;
    for script in &scripts {
        for (name, tracks) in joint_track_sets(script) {
            for (id, rig) in &rigs {
                let prepared = rig.prepare().unwrap();
                let limbs = prepared.stick_limbs().unwrap();
                for offsets in clip_poses(&prepared, &tracks) {
                    prepared
                        .check_limb_offsets(&limbs, &offsets)
                        .unwrap_or_else(|e| panic!("{id} in {name}: {e}"));
                    let world = prepared.world_transforms(&offsets).unwrap();
                    for limb in &limbs {
                        for joint in limb.moving() {
                            let gap = prepared.attachment_gap(&world, joint);
                            assert!(gap < 1e-9, "{id} in {name}: joint {joint} gap {gap}");
                        }
                    }
                    checked += 1;
                }
            }
        }
    }
    assert!(checked > 100, "only {checked} poses checked");
}

#[test]
fn a_translated_limb_joint_is_reported_as_detaching() {
    let (_, rig) = &stick_rigs()[1];
    let prepared = rig.prepare().unwrap();
    let limbs = prepared.stick_limbs().unwrap();
    let forearm = prepared.joint_index("forearm_r").unwrap();
    let offset = Transform2 {
        translation: Vec2::new(0.0, 5.0),
        ..Default::default()
    };
    assert!(prepared
        .check_limb_offsets(&limbs, &[(forearm, offset)])
        .is_err());
    let world = prepared.world_transforms(&[(forearm, offset)]).unwrap();
    assert!((prepared.attachment_gap(&world, forearm) - 5.0).abs() < 1e-9);
    // The torso is an anchor, not a limb joint: its breathing bob is allowed.
    let torso = prepared.joint_index("torso").unwrap();
    assert!(prepared
        .check_limb_offsets(&limbs, &[(torso, offset)])
        .is_ok());
}

#[test]
fn limb_ik_reaches_targets_on_every_stick_character() {
    for (id, rig) in stick_rigs() {
        let prepared = rig.prepare().unwrap();
        let limbs = prepared.stick_limbs().unwrap();
        for (limb, chain) in limbs.iter().zip(&STICK_LIMBS) {
            let world = prepared.world_transforms(&[]).unwrap();
            let root = origin(world[limb.upper]);
            for (dx, dy) in [(30.0, 40.0), (-50.0, 20.0), (10.0, -60.0), (0.0, 90.0)] {
                let target = Vec2::new(root.x + dx, root.y + dy);
                for bend in [BendDirection::Positive, BendDirection::Negative] {
                    let ik = prepared
                        .solve_limb_ik(*limb, target, bend, &[])
                        .unwrap_or_else(|e| panic!("{id} {}: {e}", chain.name));
                    assert!(ik.solution.reached, "{id} {}", chain.name);
                    let miss = (ik.end.x - target.x).hypot(ik.end.y - target.y);
                    assert!(miss < 1e-6, "{id} {} misses by {miss}", chain.name);
                    // IK only rotates, so the limb stays attached.
                    let posed = prepared.world_transforms(&ik.offsets).unwrap();
                    for joint in limb.moving() {
                        assert!(prepared.attachment_gap(&posed, joint) < 1e-9);
                    }
                    assert!(prepared.check_limb_offsets(&limbs, &ik.offsets).is_ok());
                }
            }
        }
    }
}

struct Camera {
    center: Vec2,
    zoom: f64,
    viewport: Vec2,
}
impl Camera {
    fn project(&self, p: Vec2) -> Vec2 {
        Vec2::new(
            (p.x - self.center.x) * self.zoom + 0.5 * self.viewport.x,
            (p.y - self.center.y) * self.zoom + 0.5 * self.viewport.y,
        )
    }
}

fn pair(value: &Value) -> Vec2 {
    Vec2::new(value[0].as_f64().unwrap(), value[1].as_f64().unwrap())
}

/// Two characters, Veikko with a phone in the right hand: the phone's evaluated
/// vertices match the grip transform of the hand within `tolerance_px` in every
/// IK pose, walk step and camera.
#[test]
fn phone_stays_in_hand_through_ik_walk_and_camera_change() {
    let dir = examples().join("g2-rig");
    let fixture = read_json(&dir.join("two-sticks-phone.json"));
    assert_eq!(fixture["schema_version"], 1);
    let catalog_text =
        std::fs::read_to_string(dir.join(fixture["catalog"].as_str().unwrap())).unwrap();
    let catalog = HandPropCatalog::decode(&catalog_text).unwrap();
    let prop = &fixture["prop"];
    let item = catalog
        .items
        .iter()
        .find(|i| i.id == prop["item"].as_str().unwrap())
        .expect("catalog item");
    let hand = match prop["hand"].as_str().unwrap() {
        "left" => HandSide::Left,
        _ => HandSide::Right,
    };
    let p = &prop["placement"];
    let placement = GripPlacement {
        x: p["x"].as_f64().unwrap(),
        y: p["y"].as_f64().unwrap(),
        rotation_degrees: p["rotation_degrees"].as_f64().unwrap(),
        scale: p["scale"].as_f64().unwrap(),
    };
    let tolerance = fixture["tolerance_px"].as_f64().unwrap();
    let viewport = pair(&fixture["viewport"]);
    let cameras: Vec<Camera> = fixture["cameras"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| Camera {
            center: pair(&c["center"]),
            zoom: c["zoom"].as_f64().unwrap(),
            viewport,
        })
        .collect();
    assert!(cameras.len() >= 2, "a camera change needs two cameras");

    let mut actors = Vec::new();
    for character in fixture["characters"].as_array().unwrap() {
        let id = character["id"].as_str().unwrap();
        let mut rig = rig_of(
            &script(&dir.join(character["source"].as_str().unwrap())),
            id,
        );
        if id == prop["character"].as_str().unwrap() {
            attach_hand_prop(&mut rig, hand, item, placement).unwrap();
        }
        let x = character["x"].as_f64().unwrap();
        let y = character["y"].as_f64().unwrap();
        actors.push((id, rig.prepare().unwrap(), Vec2::new(x, y)));
    }
    assert_eq!(actors.len(), 2, "two stick characters");
    let (_, holder, holder_position) = actors
        .iter()
        .find(|(id, ..)| *id == prop["character"].as_str().unwrap())
        .unwrap();
    let hand_joint = holder.joint_index(hand.joint()).unwrap();
    // Item-local vertices of the phone's case, in catalog order.
    let case = &item.parts[0];
    let case_id = format!("{}{}_{}", hand.part_prefix(), item.id, case.id);

    let ik = &fixture["ik"];
    let limbs = holder.stick_limbs().unwrap();
    let limb = STICK_LIMBS
        .iter()
        .position(|c| c.name == ik["limb"].as_str().unwrap())
        .map(|i| limbs[i])
        .unwrap();
    let bend = match ik["bend"].as_str().unwrap() {
        "negative" => BendDirection::Negative,
        _ => BendDirection::Positive,
    };
    let mut poses = vec![Vec::new()];
    let mut unreached = 0;
    for target in ik["targets"].as_array().unwrap() {
        let solved = holder.solve_limb_ik(limb, pair(target), bend, &[]).unwrap();
        unreached += usize::from(!solved.solution.reached);
        poses.push(solved.offsets.to_vec());
    }
    assert!(unreached >= 1, "fixture includes an unreachable target");

    let walk = &fixture["walk"];
    let (from, to) = (
        walk["from_x"].as_f64().unwrap(),
        walk["to_x"].as_f64().unwrap(),
    );
    let steps = walk["steps"].as_u64().unwrap();
    let mut checked = 0usize;
    for step in 0..=steps {
        let actor_x = from + (to - from) * step as f64 / steps as f64;
        let actor = Transform2 {
            translation: Vec2::new(actor_x, holder_position.y),
            ..Default::default()
        }
        .affine();
        for offsets in &poses {
            let world = holder.world_transforms(offsets).unwrap();
            let parts = holder.evaluate_parts(offsets).unwrap();
            let evaluated = parts.iter().find(|p| p.id == case_id).expect("phone case");
            assert_eq!(evaluated.points.len(), case.points.len());
            let expected_grip = placement.grip_world(world[hand_joint]);
            let hand_origin = origin(world[hand_joint]);
            for camera in &cameras {
                let screen = |p: Vec2| camera.project(actor.point(p));
                for (point, local) in evaluated.points.iter().zip(&case.points) {
                    let expected =
                        world[hand_joint].point(pair_from(placement.to_hand(item.grip, *local)));
                    let (a, b) = (screen(*point), screen(expected));
                    let error = (a.x - b.x).hypot(a.y - b.y);
                    assert!(error <= tolerance, "phone vertex off by {error}px");
                }
                // The grip point keeps its hand-local distance on screen.
                let (g, h) = (screen(expected_grip), screen(hand_origin));
                let held = (g.x - h.x).hypot(g.y - h.y);
                let local = placement.x.hypot(placement.y) * camera.zoom;
                assert!(
                    (held - local).abs() <= tolerance,
                    "grip drifted {held} vs {local}"
                );
                checked += 1;
            }
        }
    }
    assert_eq!(checked, (steps as usize + 1) * poses.len() * cameras.len());
}

fn pair_from([x, y]: [f64; 2]) -> Vec2 {
    Vec2::new(x, y)
}

#[test]
fn joint_limits_clamp_ik_and_reject_rest_outside_range() {
    let (_, mut rig) = stick_rigs().swap_remove(1);
    // Elbow may bend only one way (rest rotation 0 lies inside the range).
    rig.joint_limits.insert(
        "forearm_r".into(),
        motion_rig::JointLimitDefinition {
            min_rotation: 0.0,
            max_rotation: 2.4,
        },
    );
    let prepared = rig.prepare().unwrap();
    let limbs = prepared.stick_limbs().unwrap();
    let limb = limbs[1];
    let root = origin(prepared.world_transforms(&[]).unwrap()[limb.upper]);
    let target = Vec2::new(root.x + 40.0, root.y + 50.0);
    let results: Vec<_> = [BendDirection::Positive, BendDirection::Negative]
        .iter()
        .map(|&bend| prepared.solve_limb_ik(limb, target, bend, &[]).unwrap())
        .collect();
    // One bend direction violates the elbow limit and is clamped.
    assert_eq!(results.iter().filter(|r| r.limited).count(), 1);
    for result in &results {
        let forearm = result.offsets[1];
        let rest = prepared.rig().joints()[forearm.0].rest.rotation;
        let local = rest + forearm.1.rotation;
        assert!((0.0..=2.4).contains(&local), "elbow {local}");
        assert!(result.end.x.is_finite() && result.end.y.is_finite());
    }
    // Ordinary evaluation clamps too.
    let forearm = prepared.joint_index("forearm_r").unwrap();
    let over = Transform2 {
        rotation: 5.0,
        ..Default::default()
    };
    let max = Transform2 {
        rotation: 2.4,
        ..Default::default()
    };
    assert_eq!(
        prepared.world_transforms(&[(forearm, over)]).unwrap(),
        prepared.world_transforms(&[(forearm, max)]).unwrap()
    );
    // Limits must contain the rest rotation, be ordered and finite.
    for (min, max) in [(0.1, 1.0), (1.0, -1.0), (f64::NAN, 1.0), (-7.0, 1.0)] {
        let mut bad = rig.clone();
        bad.joint_limits.insert(
            "forearm_r".into(),
            motion_rig::JointLimitDefinition {
                min_rotation: min,
                max_rotation: max,
            },
        );
        assert!(bad.prepare().is_err(), "{min}..{max}");
    }
    let mut unknown = rig.clone();
    unknown.joint_limits.insert(
        "tail".into(),
        motion_rig::JointLimitDefinition {
            min_rotation: -1.0,
            max_rotation: 1.0,
        },
    );
    assert!(unknown.prepare().is_err());
}
