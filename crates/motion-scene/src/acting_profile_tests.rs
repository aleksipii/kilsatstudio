//! Acting profiles, emotion intensity and hand shapes on a stick-studio character.
use super::*;
use motion_rig::{ActingInput, EvaluatedPart, FaceView, HandShape};
use motion_script::Script;

const STARTER: &str = include_str!("../../../examples/stick-studio/stick-studio.kilsat");

/// A one-scene script with the stick-studio VEIKKO as `V`, `acting` as given
/// (YAML, or empty for the character's own profile) and `events`.
fn script(acting: &str, events: &str) -> Script {
    let document: serde_json::Value = serde_json::from_str(STARTER).unwrap();
    let starter = motion_script::parse(document["source"].as_str().unwrap()).unwrap();
    let mut character = starter.characters["VEIKKO"].clone();
    if !acting.is_empty() {
        character.acting = Some(serde_yaml::from_str(acting).unwrap());
    }
    // JSON is YAML flow syntax, so the character embeds as written.
    let character = serde_json::to_string(&character).unwrap();
    motion_script::parse(&format!(
        "characters: {{V: {character}}}\nscenes: [{{id: s, duration: 8, events: [{events}]}}]\n"
    ))
    .unwrap()
}

fn part<'a>(scene: &'a EvaluatedScene, id: &str) -> &'a EvaluatedPart {
    scene.actors[0]
        .rigid_parts
        .iter()
        .find(|p| p.id == id)
        .unwrap()
}

/// Polygon area: unchanged by the joint's rotation and translation, so it
/// tells the drawn shapes apart in any pose.
fn area(part: &EvaluatedPart) -> f64 {
    let p = &part.points;
    (0..p.len())
        .map(|i| {
            let (a, b) = (p[i], p[(i + 1) % p.len()]);
            a.x * b.y - b.x * a.y
        })
        .sum::<f64>()
        .abs()
        / 2.0
}

/// Area of `id` in the rest pose when drawn with `hands` and `emotion`.
fn drawn(script: &Script, id: &str, emotion: &str, hands: [Option<HandShape>; 2]) -> f64 {
    let rig = script.characters["V"]
        .rig
        .as_ref()
        .unwrap()
        .prepare()
        .unwrap();
    let input = ActingInput {
        emotion,
        blink: false,
        view: FaceView::Front,
        hands,
    };
    let (parts, _) = rig.evaluate_acting(&[], input).unwrap();
    area(parts.iter().find(|p| p.id == id).unwrap())
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-6
}

#[test]
fn gestures_emotions_and_props_choose_the_hand_shapes() {
    let s = script(
        "{expressiveness: 1}",
        "{type: emotion, at: 0, actor: V, value: angry},\
         {type: gesture, at: 2, actor: V, value: point, duration: 1.2},\
         {type: hold, at: 5, actor: V, hand: left, item: kahvikuppi},\
         {type: emotion, at: 6, actor: V, value: neutral}",
    );
    let at = |t: f64| evaluate(&s, 0, t).unwrap();
    let shape = |hand: usize, shape: HandShape| {
        let mut hands = [None, None];
        hands[hand] = Some(shape);
        drawn(&s, ["hand_l", "hand_r"][hand], "neutral", hands)
    };
    let relaxed = drawn(&s, "hand_l", "neutral", [None, None]);
    // Anger clenches both hands.
    let angry = at(1.0);
    assert!(close(
        area(part(&angry, "hand_l")),
        shape(0, HandShape::Fist)
    ));
    assert!(close(
        area(part(&angry, "hand_r")),
        shape(1, HandShape::Fist)
    ));
    // The pointing hand points while the other stays clenched.
    let pointing = at(2.6);
    assert!(close(
        area(part(&pointing, "hand_r")),
        shape(1, HandShape::Point)
    ));
    assert!(close(
        area(part(&pointing, "hand_l")),
        shape(0, HandShape::Fist)
    ));
    // A held cup is gripped even when the emotion has calmed.
    let holding = at(6.5);
    assert!(close(
        area(part(&holding, "hand_l")),
        shape(0, HandShape::Grip)
    ));
    assert!(close(area(part(&holding, "hand_r")), relaxed));
}

#[test]
fn a_mild_emotion_shows_in_the_body_but_not_the_face() {
    let mild = script(
        "{expressiveness: 1}",
        "{type: emotion, at: 0, actor: V, value: sad, intensity: 0.2}",
    );
    let full = script(
        "{expressiveness: 1}",
        "{type: emotion, at: 0, actor: V, value: sad}",
    );
    let calm = script("{expressiveness: 1}", "");
    let (mild, full, calm) = (
        evaluate(&mild, 0, 2.0).unwrap(),
        evaluate(&full, 0, 2.0).unwrap(),
        evaluate(&calm, 0, 2.0).unwrap(),
    );
    let s = script("", "");
    // The face keeps its neutral mouth at 0.2 and frowns at full intensity.
    assert!(close(
        area(part(&mild, "mouth")),
        drawn(&s, "mouth", "neutral", [None; 2])
    ));
    assert!(close(
        area(part(&full, "mouth")),
        drawn(&s, "mouth", "sad", [None; 2])
    ));
    // The head drops a little, then fully.
    let head_y = |scene: &EvaluatedScene| {
        let p = &part(scene, "face").points;
        p.iter().map(|q| q.y).sum::<f64>() / p.len() as f64
    };
    assert!(head_y(&calm) < head_y(&mild));
    assert!(head_y(&mild) < head_y(&full));
}

#[test]
fn energy_sizes_gestures_and_posture_sets_the_stance() {
    let hand_y = |acting: &str| {
        let s = script(
            acting,
            "{type: gesture, at: 0, actor: V, value: point, duration: 2}",
        );
        let scene = evaluate(&s, 0, 1.0).unwrap();
        let p = &part(&scene, "hand_r").points;
        p.iter().map(|q| q.y).sum::<f64>() / p.len() as f64
    };
    // The pointing arm swings higher (smaller y) with more energy.
    assert!(hand_y("{energy: 1.5}") < hand_y("{energy: 1}"));
    assert!(hand_y("{energy: 1}") < hand_y("{energy: 0.5}"));
    let head_y = |acting: &str| {
        let s = script(acting, "");
        let scene = evaluate(&s, 0, 1.0).unwrap();
        let p = &part(&scene, "face").points;
        p.iter().map(|q| q.y).sum::<f64>() / p.len() as f64
    };
    assert!(head_y("{posture: 1}") < head_y("{posture: 0}"));
    assert!(head_y("{posture: 0}") < head_y("{posture: -1}"));
}

#[test]
fn acting_layers_do_not_depend_on_seek_order() {
    let s = script(
        "nervous",
        "{type: emotion, at: 0.5, actor: V, value: surprised, intensity: 0.7},\
         {type: gesture, at: 1, actor: V, value: shrug, duration: 1.5},\
         {type: speak, at: 3, actor: V, text: Hei, emotion: angry, intensity: 0.4}",
    );
    let validated = ValidatedScript::new(std::sync::Arc::new(s.clone())).unwrap();
    let times: Vec<f64> = (0..96).map(|i| f64::from(i) / 24.0).collect();
    let forward: Vec<_> = times
        .iter()
        .map(|t| validated.evaluate(0, *t).unwrap())
        .collect();
    for (i, t) in times.iter().enumerate().rev() {
        assert_eq!(validated.evaluate(0, *t).unwrap(), forward[i], "t={t}");
    }
    // Every stick-studio character carries a valid acting profile.
    let document: serde_json::Value = serde_json::from_str(STARTER).unwrap();
    let starter = motion_script::parse(document["source"].as_str().unwrap()).unwrap();
    for (id, character) in &starter.characters {
        assert!(character.acting.is_some(), "{id}");
        motion_script::character_acting(character.acting.as_ref(), id).unwrap();
    }
}
