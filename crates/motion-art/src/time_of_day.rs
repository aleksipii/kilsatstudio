//! Time of day for a scene: a preset of scene lights over the whole picture.
//!
//! Morning, evening and night are an ambient multiply tint plus a vignette.
//! Lights are drawn above the artwork and the characters, so the set and the
//! cast take the same colour. Day removes the preset. The lights are ordinary
//! `Light`s with ids `tod/<time>/<part>/<scene id>`, editable like any other.
use crate::{Color, Document, Error, Light, LightSource};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeOfDay {
    #[default]
    Day,
    Morning,
    Evening,
    Night,
}

/// Story and editor names, in the order of a day.
pub const TIMES_OF_DAY: [&str; 4] = ["day", "morning", "evening", "night"];
const PREFIX: &str = "tod/";
/// Ambient colour and strength, vignette strength and clear inner fraction.
type Grade = ((u8, u8, u8), f64, f64, f64);

impl TimeOfDay {
    pub const ALL: [TimeOfDay; 4] = [
        TimeOfDay::Day,
        TimeOfDay::Morning,
        TimeOfDay::Evening,
        TimeOfDay::Night,
    ];
    pub fn name(self) -> &'static str {
        TIMES_OF_DAY[self as usize]
    }
    pub fn parse(name: &str) -> Option<Self> {
        TIMES_OF_DAY
            .iter()
            .position(|n| *n == name)
            .map(|i| Self::ALL[i])
    }
    /// Finnish label for editors.
    pub fn label(self) -> &'static str {
        match self {
            TimeOfDay::Day => "Päivä",
            TimeOfDay::Morning => "Aamu",
            TimeOfDay::Evening => "Ilta",
            TimeOfDay::Night => "Yö",
        }
    }
    /// `(ambient colour, ambient strength, vignette strength, vignette inner)`.
    fn grade(self) -> Option<Grade> {
        match self {
            TimeOfDay::Day => None,
            TimeOfDay::Morning => Some(((255, 228, 196), 0.28, 0.18, 0.7)),
            TimeOfDay::Evening => Some(((255, 172, 118), 0.42, 0.36, 0.55)),
            TimeOfDay::Night => Some(((58, 78, 140), 0.74, 0.55, 0.42)),
        }
    }
    /// The preset lights of this time for `scene_id` (none for day).
    pub fn lights(self, scene_id: &str) -> Vec<Light> {
        let Some(((r, g, b), ambient, vignette, inner)) = self.grade() else {
            return Vec::new();
        };
        let light = |part: &str, source| Light {
            id: format!("{PREFIX}{}/{part}/{scene_id}", self.name()),
            scene_id: scene_id.to_string(),
            visible: true,
            active: None,
            flicker: None,
            behind_actors: false,
            source,
        };
        vec![
            light(
                "ambient",
                LightSource::Ambient {
                    color: Color { r, g, b, a: 255 },
                    strength: ambient,
                },
            ),
            light(
                "vignette",
                LightSource::Vignette {
                    strength: vignette,
                    inner,
                },
            ),
        ]
    }
}

fn preset_time(light: &Light) -> Option<TimeOfDay> {
    let rest = light.id.strip_prefix(PREFIX)?;
    TimeOfDay::parse(rest.split('/').next()?)
}

/// The time of day a scene's preset lights show (day when there are none).
pub fn scene_time_of_day(document: &Document, scene_id: &str) -> TimeOfDay {
    document
        .lights
        .iter()
        .filter(|l| l.scene_id == scene_id)
        .find_map(preset_time)
        .unwrap_or_default()
}

/// `document` with the scene's preset lights replaced by those of `time`.
/// Other lights stay. The result is validated (16 lights per scene at most).
pub fn with_time_of_day(
    document: &Document,
    scene_id: &str,
    time: TimeOfDay,
) -> Result<Document, Error> {
    let mut next = document.clone();
    next.lights
        .retain(|l| !(l.scene_id == scene_id && preset_time(l).is_some()));
    next.lights.extend(time.lights(scene_id));
    next.validate()?;
    Ok(next)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_replace_each_other_and_day_clears_them() {
        let doc = Document::default();
        assert_eq!(scene_time_of_day(&doc, "s"), TimeOfDay::Day);
        let night = with_time_of_day(&doc, "s", TimeOfDay::Night).unwrap();
        assert_eq!(night.lights.len(), 2);
        assert_eq!(scene_time_of_day(&night, "s"), TimeOfDay::Night);
        assert_eq!(scene_time_of_day(&night, "other"), TimeOfDay::Day);
        let evening = with_time_of_day(&night, "s", TimeOfDay::Evening).unwrap();
        assert_eq!(evening.lights.len(), 2);
        assert_eq!(scene_time_of_day(&evening, "s"), TimeOfDay::Evening);
        let both = with_time_of_day(&evening, "t", TimeOfDay::Morning).unwrap();
        assert_eq!(both.lights.len(), 4);
        let day = with_time_of_day(&both, "s", TimeOfDay::Day).unwrap();
        assert_eq!(day.lights.len(), 2);
        assert_eq!(scene_time_of_day(&day, "t"), TimeOfDay::Morning);
    }

    #[test]
    fn names_round_trip_and_night_is_darker_and_bluer_than_evening() {
        for time in TimeOfDay::ALL {
            assert_eq!(TimeOfDay::parse(time.name()), Some(time));
        }
        assert!(TimeOfDay::parse("noon").is_none());
        let ambient = |time: TimeOfDay| match time.lights("s")[0].source {
            LightSource::Ambient { color, strength } => (color, strength),
            _ => unreachable!(),
        };
        let (night, night_strength) = ambient(TimeOfDay::Night);
        let (evening, evening_strength) = ambient(TimeOfDay::Evening);
        assert!(night.b > night.r && evening.r > evening.b);
        assert!(night_strength > evening_strength);
    }

    #[test]
    fn other_lights_are_kept() {
        let mut doc = with_time_of_day(&Document::default(), "s", TimeOfDay::Night).unwrap();
        let mut lamp = TimeOfDay::Night.lights("s")[0].clone();
        lamp.id = "lamppu".into();
        doc.lights.push(lamp);
        let day = with_time_of_day(&doc, "s", TimeOfDay::Day).unwrap();
        assert_eq!(day.lights.len(), 1);
        assert_eq!(day.lights[0].id, "lamppu");
    }
}
