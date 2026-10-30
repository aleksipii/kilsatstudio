//! Plain dialogue to a new, editable scene; no synthesis or file side effects.
use crate::project_history::MAX_SOURCE_BYTES;
use motion_script::{Event, Scene};

pub struct Draft {
    pub dialogue: String,
    pub seconds_per_line: f64,
}

impl Default for Draft {
    fn default() -> Self {
        Self {
            dialogue: "EMMA: Hello Alex!\nALEX: Let's make an animation.".into(),
            seconds_per_line: 5.0,
        }
    }
}

pub fn append_scene(source: &str, draft: &Draft) -> Result<(String, usize), String> {
    if draft.dialogue.len() > 16_000 {
        return Err("Vuorosanojen enimmäiskoko on 16 KB.".into());
    }
    if !draft.seconds_per_line.is_finite() || !(1.0..=30.0).contains(&draft.seconds_per_line) {
        return Err("Valitse repliikin kestoksi 1–30 sekuntia.".into());
    }
    let script = motion_script::parse(source).map_err(|e| e.to_string())?;
    let mut events = Vec::new();
    let mut speakers = Vec::new();
    for (index, line) in draft.dialogue.lines().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        let (actor, text) = line
            .split_once(':')
            .ok_or_else(|| format!("Rivi {}: käytä muotoa HAHMO: vuorosana", index + 1))?;
        let actor = actor.trim();
        let text = text.trim();
        if !script.characters.contains_key(actor) {
            return Err(format!("Rivi {}: tuntematon hahmo {actor}", index + 1));
        }
        if text.is_empty() {
            return Err(format!("Rivi {}: vuorosana puuttuu", index + 1));
        }
        if !speakers.iter().any(|speaker| speaker == actor) {
            speakers.push(actor.to_string());
        }
        if events.len() >= 60 {
            return Err("Kohtauksessa voi olla enintään 60 repliikkiä.".into());
        }
        events.push(Event::Speak {
            at: 1.0 + events.len() as f64 * draft.seconds_per_line,
            actor: actor.into(),
            text: text.into(),
            emotion: None,
            audio: None,
        });
    }
    if events.is_empty() {
        return Err("Kirjoita vähintään yksi repliikki.".into());
    }
    let duration = 2.0 + events.len() as f64 * draft.seconds_per_line;
    for (index, actor) in speakers.iter().enumerate() {
        events.push(Event::Move {
            at: 0.0,
            actor: actor.clone(),
            x: f64::from(script.project.width) * (index + 1) as f64 / (speakers.len() + 1) as f64,
            y: f64::from(script.project.height) * 0.70,
            duration: 0.5,
            gait: None,
            ease: None,
        });
        events.push(Event::Emotion {
            at: 0.0,
            actor: actor.clone(),
            value: "happy".into(),
        });
    }
    let mut number = script.scenes.len() + 1;
    while script
        .scenes
        .iter()
        .any(|s| s.id == format!("dialogue_{number}"))
    {
        number += 1;
    }
    let mut composition = motion_script::Composition::default();
    for actor in script.characters.keys() {
        composition.layers.insert(
            actor.clone(),
            motion_script::Layer {
                z: 0,
                visible: speakers.contains(actor),
            },
        );
    }
    let scene = Scene {
        composition,
        background: Some("studio".into()),
        id: format!("dialogue_{number}"),
        duration,
        events,
    };
    let updated = motion_script::add_scene(source, scene).map_err(|e| e.to_string())?;
    if updated.len() > MAX_SOURCE_BYTES {
        return Err("Tulos ylittää työpöytäsovelluksen käsikirjoituksen 256 KiB:n rajan.".into());
    }
    Ok((updated, script.scenes.len()))
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = include_str!("../../../examples/hello-emma.yaml");

    #[test]
    fn appends_editable_scene_without_replacing_existing_work() {
        let (source, selected) = append_scene(SOURCE, &Draft::default()).unwrap();
        let script = motion_script::parse(&source).unwrap();
        assert_eq!(selected, 1);
        assert_eq!(script.scenes[0].id, "intro");
        assert_eq!(script.scenes[1].duration, 12.0);
        let lines: Vec<_> = script.scenes[1]
            .events
            .iter()
            .filter_map(|event| {
                if let Event::Speak { at, text, .. } = event {
                    Some((*at, text.as_str()))
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(
            lines,
            vec![(1.0, "Hello Alex!"), (6.0, "Let's make an animation.")]
        );
        let (again, _) = append_scene(&source, &Draft::default()).unwrap();
        assert_eq!(
            motion_script::parse(&again).unwrap().scenes[2].id,
            "dialogue_3"
        );
    }

    #[test]
    fn rejects_invalid_input_without_partial_scene() {
        for dialogue in ["", "EMMA no colon", "MISSING: hi", "EMMA:"] {
            assert!(append_scene(
                SOURCE,
                &Draft {
                    dialogue: dialogue.into(),
                    ..Draft::default()
                }
            )
            .is_err());
        }
        for seconds_per_line in [0.0, f64::NAN, f64::INFINITY, 31.0] {
            assert!(append_scene(
                SOURCE,
                &Draft {
                    seconds_per_line,
                    ..Draft::default()
                }
            )
            .is_err());
        }
        assert!(append_scene(
            SOURCE,
            &Draft {
                dialogue: "EMMA: hi\n".repeat(61),
                ..Draft::default()
            }
        )
        .is_err());
    }

    #[test]
    fn keeps_colons_unicode_and_empty_lines_in_dialogue() {
        let draft = Draft {
            dialogue: "\n EMMA: Hei: maailma! 🎬\n\n".into(),
            ..Draft::default()
        };
        let (source, _) = append_scene(SOURCE, &draft).unwrap();
        let script = motion_script::parse(&source).unwrap();
        assert!(!script.scenes[1].composition.layers["ALEX"].visible);
        assert!(script.scenes[1].composition.layers["EMMA"].visible);
        assert!(
            matches!(&script.scenes[1].events[0], Event::Speak { text, .. } if text == "Hei: maailma! 🎬")
        );
    }
}
