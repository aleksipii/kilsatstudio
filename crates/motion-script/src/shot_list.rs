//! The shots of a screenplay, for the storyboard, the animatic and shot status.
//!
//! A shot starts at every scene start, at every hard cut (two camera keys less
//! than half a frame apart, which is how the story compiler and the shot
//! library write a cut) and at every camera-angle change (angles never blend).
//! Within a scene the shots are numbered 010, 020, … as in a studio shot list,
//! and a shot's id is `<scene id>/<number>`.
//!
//! The list is a pure function of the script: the same screenplay always gives
//! the same shots, times and panel moments.
use crate::{Event, Script};

/// Most shots listed for one screenplay.
pub const MAX_LISTED_SHOTS: usize = 2048;

/// One line of dialogue inside a shot.
#[derive(Debug, Clone, PartialEq)]
pub struct ShotDialogue {
    pub at: f64,
    pub actor: String,
    pub text: String,
    pub emotion: Option<String>,
}

/// One action inside a shot: `kind` is the event type (`gesture`, `emotion`,
/// `move`, `pose`, `face`, `hold`), `value` its value (`wave`, `sad 0.4`, …).
#[derive(Debug, Clone, PartialEq)]
pub struct ShotAction {
    pub at: f64,
    pub actor: String,
    pub kind: &'static str,
    pub value: String,
}

/// Framing class of a shot from its camera zoom.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShotSize {
    /// Zoom below 1.25.
    Wide,
    /// Zoom 1.25–1.95 (medium shots and two-shots).
    Medium,
    /// Zoom 1.95 and above (reactions and close-ups).
    CloseUp,
}
impl ShotSize {
    pub fn from_zoom(zoom: f64) -> Self {
        if zoom < 1.25 {
            Self::Wide
        } else if zoom < 1.95 {
            Self::Medium
        } else {
            Self::CloseUp
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Wide => "wide",
            Self::Medium => "medium",
            Self::CloseUp => "close_up",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ShotInfo {
    /// `<scene id>/<number>`, for example `kahvila_aamu/020`.
    pub id: String,
    pub scene_index: usize,
    pub scene_id: String,
    /// 10, 20, 30, … within the scene.
    pub number: u32,
    /// Scene-local start and end in seconds.
    pub start: f64,
    pub end: f64,
    /// Scene-local moment drawn as the storyboard panel and held in the animatic.
    pub panel: f64,
    pub size: ShotSize,
    pub zoom: f64,
    pub angle: String,
    pub lines: Vec<ShotDialogue>,
    pub actions: Vec<ShotAction>,
}
impl ShotInfo {
    pub fn duration(&self) -> f64 {
        self.end - self.start
    }
}

fn shot_id(scene: &str, number: u32) -> String {
    format!("{scene}/{number:03}")
}

/// Every shot of `script`, scene by scene. Scenes without camera keys are one
/// shot each. More than [`MAX_LISTED_SHOTS`] shots is an error.
pub fn shot_list(script: &Script) -> Result<Vec<ShotInfo>, crate::Error> {
    let fps = f64::from(script.project.fps.max(1));
    let gap = 0.5 / fps;
    let mut shots = Vec::new();
    for (scene_index, scene) in script.scenes.iter().enumerate() {
        let mut keys = scene.composition.camera.clone();
        keys.sort_by(|a, b| a.at.total_cmp(&b.at));
        let mut cuts = vec![0.0];
        for pair in keys.windows(2) {
            let (before, key) = (&pair[0], &pair[1]);
            if key.at - before.at <= gap || key.angle() != before.angle() {
                cuts.push(key.at);
            }
        }
        // A cut at (or within half a frame of) the start or end is not a shot.
        cuts.retain(|t| *t == 0.0 || (*t > gap && *t < scene.duration - gap));
        cuts.dedup_by(|b, a| *b - *a <= gap);
        for (i, &start) in cuts.iter().enumerate() {
            let end = cuts.get(i + 1).copied().unwrap_or(scene.duration);
            let inside = |at: f64| at >= start && at < end;
            let mut lines = Vec::new();
            let mut actions = Vec::new();
            for event in &scene.events {
                let (at, actor) = event.actor_time();
                if !inside(at) {
                    continue;
                }
                let actor = actor.to_string();
                let (kind, value) = match event {
                    Event::Speak { text, emotion, .. } => {
                        lines.push(ShotDialogue {
                            at,
                            actor,
                            text: text.clone(),
                            emotion: emotion.clone(),
                        });
                        continue;
                    }
                    Event::Gesture { value, .. } => ("gesture", value.clone()),
                    Event::Emotion {
                        value, intensity, ..
                    } => (
                        "emotion",
                        match intensity {
                            Some(i) => format!("{value} {i}"),
                            None => value.clone(),
                        },
                    ),
                    Event::Move { gait, .. } => {
                        ("move", gait.clone().unwrap_or_else(|| "walk".into()))
                    }
                    Event::Pose { value, .. } => ("pose", value.clone()),
                    Event::Face { value, .. } => ("face", value.clone()),
                    Event::Hold { item, .. } => {
                        ("hold", item.clone().unwrap_or_else(|| "none".into()))
                    }
                };
                actions.push(ShotAction {
                    at,
                    actor,
                    kind,
                    value,
                });
            }
            lines.sort_by(|a, b| a.at.total_cmp(&b.at));
            actions.sort_by(|a, b| a.at.total_cmp(&b.at));
            let panel = panel_time(scene, start, end, fps);
            // The framing that holds at the panel (the key at or before it).
            let key = keys
                .iter()
                .rev()
                .find(|k| k.at <= panel)
                .or_else(|| keys.first());
            let zoom = key.map_or(1.0, |k| k.zoom);
            let number = 10 * (i as u32 + 1);
            shots.push(ShotInfo {
                id: shot_id(&scene.id, number),
                scene_index,
                scene_id: scene.id.clone(),
                number,
                start,
                end,
                panel,
                size: ShotSize::from_zoom(zoom),
                zoom,
                angle: key.map_or("front", |k| k.angle()).to_string(),
                lines,
                actions,
            });
            if shots.len() > MAX_LISTED_SHOTS {
                return Err(crate::Error::Validation(format!(
                    "the screenplay has more than {MAX_LISTED_SHOTS} shots"
                )));
            }
        }
    }
    Ok(shots)
}

/// The panel moment: the peak of the first gesture, else just into the first
/// line, else the middle of the first move, else the middle of the shot. On a
/// frame, inside the shot.
fn panel_time(scene: &crate::Scene, start: f64, end: f64, fps: f64) -> f64 {
    let inside = |at: f64| at >= start && at < end;
    let first = |pick: &dyn Fn(&Event) -> Option<f64>| {
        scene
            .events
            .iter()
            .filter(|e| inside(e.actor_time().0))
            .filter_map(pick)
            .min_by(f64::total_cmp)
    };
    let moment = first(&|e| match e {
        Event::Gesture { at, duration, .. } => Some(at + 0.4 * duration),
        _ => None,
    })
    .or_else(|| {
        first(&|e| match e {
            Event::Speak { at, .. } => Some(at + 0.4),
            _ => None,
        })
    })
    .or_else(|| {
        first(&|e| match e {
            Event::Move { at, duration, .. } => Some(at + duration / 2.0),
            _ => None,
        })
    })
    .unwrap_or((start + end) / 2.0);
    let last = ((end * fps).ceil() - 1.0).max((start * fps).ceil()) / fps;
    let frame = (moment.clamp(start, last) * fps).round() / fps;
    frame.clamp(start, last.max(start))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script(scenes: &str) -> Script {
        crate::parse(&format!(
            "project: {{fps: 24}}\ncharacters: {{A: {{asset: a, voice: {{provider: kokoro, preset: af_heart, language: en-us, speed: 1}}}}}}\nscenes: {scenes}\n"
        ))
        .unwrap()
    }

    #[test]
    fn cuts_angles_and_scenes_start_shots() {
        let s = script(
            "[{id: one, duration: 10, events: [{type: gesture, at: 4.5, actor: A, value: wave, duration: 1}, {type: speak, at: 7, actor: A, text: Hello}], composition: {camera: [\
             {at: 0, x: 0, y: 0, zoom: 1}, {at: 3.999, x: 0, y: 0, zoom: 1}, {at: 4, x: 50, y: 0, zoom: 2.4},\
             {at: 5, x: 60, y: 0, zoom: 2.5}, {at: 6, x: 0, y: 0, zoom: 1.5, angle: reverse}]}},\
             {id: two, duration: 2, events: []}]",
        );
        let shots = shot_list(&s).unwrap();
        let ids: Vec<_> = shots.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["one/010", "one/020", "one/030", "two/010"]);
        let spans: Vec<_> = shots.iter().map(|s| (s.start, s.end)).collect();
        assert_eq!(spans, [(0.0, 4.0), (4.0, 6.0), (6.0, 10.0), (0.0, 2.0)]);
        // A camera move inside a shot (4 → 5) is not a cut.
        assert_eq!(shots[1].size, ShotSize::CloseUp);
        assert_eq!(shots[0].size, ShotSize::Wide);
        assert_eq!(
            (shots[2].size, shots[2].angle.as_str()),
            (ShotSize::Medium, "reverse")
        );
        // Panels: the gesture peak, just into the line, the middle of a quiet shot.
        assert!(
            (shots[1].panel - 118.0 / 24.0).abs() < 1e-9,
            "{}",
            shots[1].panel
        );
        assert!((shots[2].panel - 178.0 / 24.0).abs() < 1e-9);
        assert!((shots[3].panel - 1.0).abs() < 1e-9);
        assert_eq!(shots[2].lines[0].text, "Hello");
        assert_eq!(shots[1].actions[0].kind, "gesture");
        assert_eq!(shots[0].actions.len(), 0);
    }

    #[test]
    fn panels_land_on_frames_inside_short_shots() {
        let s = script(
            "[{id: one, duration: 1, events: [{type: gesture, at: 0.9, actor: A, value: wave, duration: 0.1}], composition: {camera: [{at: 0, x: 0, y: 0, zoom: 1}, {at: 0.5, x: 0, y: 0, zoom: 1}, {at: 0.5001, x: 0, y: 0, zoom: 2}]}}]",
        );
        for shot in shot_list(&s).unwrap() {
            assert!(
                shot.panel >= shot.start && shot.panel < shot.end,
                "{shot:?}"
            );
            let frames = shot.panel * 24.0;
            assert!((frames - frames.round()).abs() < 1e-9);
        }
    }

    #[test]
    fn the_story_example_lists_its_cuts_deterministically() {
        let starter: serde_json::Value = serde_json::from_str(include_str!(
            "../../../examples/stick-studio/stick-studio.kilsat"
        ))
        .unwrap();
        let script = crate::parse(starter["source"].as_str().unwrap()).unwrap();
        let first = shot_list(&script).unwrap();
        assert_eq!(first, shot_list(&script).unwrap());
        assert!(first.len() >= script.scenes.len());
        for pair in first.windows(2) {
            if pair[0].scene_index == pair[1].scene_index {
                assert_eq!(pair[0].end, pair[1].start);
                assert!(pair[1].number > pair[0].number);
            }
        }
    }
}
