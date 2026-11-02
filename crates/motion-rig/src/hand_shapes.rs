//! Hand shapes: drawn alternatives of a rig's hand parts (fist, open palm,
//! pointing finger, grip, thumbs up).
//!
//! A rig opts in with `hands:`, mapping shape names to variants of its hand
//! parts. Scene evaluation chooses a shape per hand (from the gesture, the
//! emotion and what the hand holds) and passes it in `ActingInput::hands`.
//! A hand without a chosen shape, or without a variant for it, keeps its base
//! drawing, which is the relaxed hand.
//!
//! ```yaml
//! hands:
//!   left: {part: hand_l, shapes: {fist: fist, open: open, point: point}}
//!   right: {part: hand_r, shapes: {fist: fist, open: open, grip: grip}}
//! ```
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// A drawn hand shape. The relaxed hand is the part's base drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum HandShape {
    Fist,
    Open,
    Point,
    Grip,
    ThumbsUp,
}
/// Shape names accepted in `hands.*.shapes`, in `HandShape` order.
pub const HAND_SHAPE_NAMES: [&str; 5] = ["fist", "open", "point", "grip", "thumbs_up"];
impl HandShape {
    pub const ALL: [HandShape; 5] = [
        HandShape::Fist,
        HandShape::Open,
        HandShape::Point,
        HandShape::Grip,
        HandShape::ThumbsUp,
    ];
    pub fn name(self) -> &'static str {
        HAND_SHAPE_NAMES[self as usize]
    }
    pub fn parse(name: &str) -> Option<Self> {
        HAND_SHAPE_NAMES
            .iter()
            .position(|n| *n == name)
            .map(|i| Self::ALL[i])
    }
}

/// `left` and `right` are each optional; a rig may draw shapes for one hand.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandsDefinition {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub left: Option<HandShapesDefinition>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub right: Option<HandShapesDefinition>,
}
/// `part` is the hand part; `shapes` sends shape names to variant ids of it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HandShapesDefinition {
    pub part: String,
    pub shapes: BTreeMap<String, String>,
}
