//! Which drawn hand shape each hand shows at one moment.
//!
//! Priority, highest first: a hand holding a prop grips it; an active gesture
//! shapes the hands it uses (a point points, a wave opens the palm); a strong
//! emotion shapes both hands (anger clenches fists, surprise opens them).
//! Otherwise the hand keeps the rig's relaxed base drawing. A shape the rig
//! does not draw falls back to its fist, and otherwise to the base drawing.
use motion_animation::Gesture;
use motion_rig::{HandShape, PreparedRig};

/// Gesture envelope weight from which the gesture's hand shapes show.
pub(crate) const GESTURE_HAND_WEIGHT: f64 = 0.35;
/// Effective emotion intensity from which anger and surprise shape the hands.
pub(crate) const EMOTION_HAND_INTENSITY: f64 = 0.5;

/// Shapes `[left, right]` a gesture gives the hands it uses. Gestures that
/// do not use the hands leave them to the emotion.
fn gesture_hands(gesture: Gesture) -> [Option<HandShape>; 2] {
    use HandShape::*;
    match gesture {
        Gesture::Wave | Gesture::Facepalm => [None, Some(Open)],
        Gesture::Point => [None, Some(Point)],
        Gesture::ThumbsUp => [None, Some(ThumbsUp)],
        Gesture::Think => [None, Some(Fist)],
        Gesture::ArmsCrossed => [Some(Fist), Some(Fist)],
        Gesture::Shrug | Gesture::Clap | Gesture::Stumble | Gesture::Jump => {
            [Some(Open), Some(Open)]
        }
        Gesture::Nod | Gesture::ShakeHead | Gesture::Bow => [None, None],
    }
}

/// The hand shapes to draw. `gesture` is the active gesture and its envelope
/// weight; `intensity` the effective emotion intensity; `holding` whether
/// each hand `[left, right]` carries a prop now.
pub(crate) fn hand_shapes(
    rig: &PreparedRig,
    gesture: Option<(Gesture, f64)>,
    emotion: &str,
    intensity: f64,
    holding: [bool; 2],
) -> [Option<HandShape>; 2] {
    let mut shapes = match emotion {
        "angry" if intensity >= EMOTION_HAND_INTENSITY => [Some(HandShape::Fist); 2],
        "surprised" if intensity >= EMOTION_HAND_INTENSITY => [Some(HandShape::Open); 2],
        _ => [None, None],
    };
    if let Some((gesture, weight)) = gesture {
        if weight >= GESTURE_HAND_WEIGHT {
            for (slot, shape) in shapes.iter_mut().zip(gesture_hands(gesture)) {
                if shape.is_some() {
                    *slot = shape;
                }
            }
        }
    }
    for (slot, held) in shapes.iter_mut().zip(holding) {
        if held {
            *slot = Some(HandShape::Grip);
        }
    }
    for (hand, slot) in shapes.iter_mut().enumerate() {
        if let Some(shape) = *slot {
            if !rig.draws_hand_shape(hand, shape) {
                *slot = (shape != HandShape::Open && rig.draws_hand_shape(hand, HandShape::Fist))
                    .then_some(HandShape::Fist);
            }
        }
    }
    shapes
}

#[cfg(test)]
mod tests {
    use super::*;
    use motion_rig::RigDefinition;

    fn rig(shapes: &[&str]) -> PreparedRig {
        let hand = |side: &str| {
            let variants: String = shapes
                .iter()
                .map(|s| format!("{s}: {{color: [1, 2, 3, 255]}}, "))
                .collect();
            let map: String = shapes.iter().map(|s| format!("{s}: {s}, ")).collect();
            (
                format!("{{id: hand_{side}, joint: hand_{side}, points: [[-5, 0], [5, 0], [0, 9]], color: [0, 0, 0, 255], variants: {{{variants}}}}}"),
                format!("{{part: hand_{side}, shapes: {{{map}}}}}"),
            )
        };
        let (left, left_map) = hand("l");
        let (right, right_map) = hand("r");
        let source = format!(
            "{{version: 1, id: t, joints: [{{id: root, parent: null}}, {{id: hand_l, parent: root}}, {{id: hand_r, parent: root}}], parts: [{left}, {right}], hands: {{left: {left_map}, right: {right_map}}}}}"
        );
        let definition: RigDefinition = serde_yaml::from_str(&source).unwrap();
        definition.prepare().unwrap()
    }

    const ALL: [&str; 5] = ["fist", "open", "point", "grip", "thumbs_up"];

    #[test]
    fn grip_beats_gesture_beats_emotion() {
        let r = rig(&ALL);
        let point = Some((Gesture::Point, 1.0));
        assert_eq!(
            hand_shapes(&r, point, "angry", 1.0, [false, false]),
            [Some(HandShape::Fist), Some(HandShape::Point)]
        );
        assert_eq!(
            hand_shapes(&r, point, "angry", 1.0, [false, true]),
            [Some(HandShape::Fist), Some(HandShape::Grip)]
        );
        // A gesture still easing in leaves the hands to the emotion.
        assert_eq!(
            hand_shapes(&r, Some((Gesture::Point, 0.2)), "neutral", 1.0, [false; 2]),
            [None, None]
        );
    }

    #[test]
    fn a_mild_emotion_leaves_the_hands_relaxed() {
        let r = rig(&ALL);
        assert_eq!(
            hand_shapes(&r, None, "angry", 0.4, [false; 2]),
            [None, None]
        );
        assert_eq!(
            hand_shapes(&r, None, "surprised", 0.8, [false; 2]),
            [Some(HandShape::Open); 2]
        );
        assert_eq!(
            hand_shapes(&r, None, "happy", 1.0, [false; 2]),
            [None, None]
        );
    }

    #[test]
    fn undrawn_shapes_fall_back_to_the_fist_or_the_base() {
        let r = rig(&["fist"]);
        assert_eq!(
            hand_shapes(
                &r,
                Some((Gesture::ThumbsUp, 1.0)),
                "neutral",
                1.0,
                [true, false]
            ),
            [Some(HandShape::Fist), Some(HandShape::Fist)]
        );
        assert_eq!(
            hand_shapes(&r, Some((Gesture::Clap, 1.0)), "neutral", 1.0, [false; 2]),
            [None, None]
        );
    }
}
