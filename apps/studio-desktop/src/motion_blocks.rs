//! Motion blocks: ready-made actions dragged from the palette onto a
//! character's timeline track. A drop becomes ordinary MotionScript events, so
//! the result stays editable, undoable and validated like any other edit.
use motion_script::{Event, Script};

/// One palette entry. Gait, gesture and facing names are MotionScript values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Gait(&'static str),
    Gesture(&'static str),
    Face(&'static str),
    /// A saved clip from the project's clip library, by name.
    Clip(String),
}

/// Built-in blocks in palette order, grouped by heading.
pub fn builtin() -> [(&'static str, Vec<Block>); 3] {
    [
        (
            "Liikkuminen",
            motion_scene::GAITS.iter().map(|g| Block::Gait(g)).collect(),
        ),
        (
            "Eleet",
            motion_scene::GESTURES
                .iter()
                .map(|g| Block::Gesture(g))
                .collect(),
        ),
        (
            "Suunta",
            motion_scene::FACINGS
                .iter()
                .map(|f| Block::Face(f))
                .collect(),
        ),
    ]
}

/// Saved clips that fit a character: rig clips for custom rigs, otherwise
/// original-rig animation clips.
pub fn saved(script: &Script, actor: Option<&str>) -> Vec<Block> {
    let custom = actor
        .and_then(|a| script.characters.get(a))
        .is_some_and(|c| c.rig.is_some());
    if custom {
        script.rig_clips.keys().cloned().map(Block::Clip).collect()
    } else {
        script
            .animation_clips
            .keys()
            .cloned()
            .map(Block::Clip)
            .collect()
    }
}

impl Block {
    pub fn label(&self) -> String {
        match self {
            Self::Gait(g) => crate::event_editor::gait_label(g).to_owned(),
            Self::Gesture(g) => crate::event_editor::gesture_label(g).to_owned(),
            Self::Face(f) => crate::event_editor::facing_label(f).to_owned(),
            Self::Clip(name) => name.clone(),
        }
    }
    pub fn hint(&self) -> &'static str {
        match self {
            Self::Gait(_) => "Vedä hahmon raidalle. Hahmo liikkuu kohti näyttämön keskustaa ja kääntyy kulkusuuntaan. Muuta määränpäätä vetämällä hahmoa näyttämöllä, pituutta palkin oikeasta reunasta.",
            Self::Gesture(_) => "Vedä hahmon raidalle. Muuta kestoa palkin oikeasta reunasta.",
            Self::Face(_) => "Vedä hahmon raidalle. Hahmo kääntyy tästä kohdasta alkaen, kunnes seuraava suuntapalikka kääntää sen.",
            Self::Clip(_) => "Tallennettu liike. Vedä hahmon raidalle; liike lisätään nivelten avainruutuina.",
        }
    }
}

/// Default length of a dropped gait or gesture, in seconds.
pub const DEFAULT_SECONDS: f64 = 1.5;
/// Gait speed as a fraction of the canvas width per second.
fn speed(gait: &str) -> f64 {
    match gait {
        "sprint" => 0.6,
        "run" => 0.45,
        "leap" => 0.3,
        "jog" => 0.27,
        "stroll" => 0.11,
        "sneak" | "tiptoe" | "limp" | "backwards" | "carry" => 0.09,
        _ => 0.16,
    }
}

fn frame_snap(script: &Script, scene_index: usize, at: f64) -> Result<f64, String> {
    if !at.is_finite() {
        return Err("Pudotuskohta ei ole kelvollinen aika.".into());
    }
    let scene = script
        .scenes
        .get(scene_index)
        .ok_or("Valitse ensin kohtaus.")?;
    let rate = f64::from(script.project.fps);
    let last = ((scene.duration * rate).ceil() - 1.0).max(0.0);
    let at = (at * rate).round().clamp(0.0, last) / rate;
    if at >= scene.duration {
        return Err("Pudotuskohta on kohtauksen lopussa.".into());
    }
    Ok(at)
}

/// The facing in effect for `actor` at `at` from its face events.
fn facing_at<'a>(events: &'a [Event], actor: &str, at: f64) -> &'a str {
    let mut best: Option<(f64, &str)> = None;
    for event in events {
        if let Event::Face {
            at: t,
            actor: a,
            value,
        } = event
        {
            if a == actor && *t <= at && best.is_none_or(|(b, _)| *t >= b) {
                best = Some((*t, value));
            }
        }
    }
    best.map_or("front", |(_, value)| value)
}

/// Events for a built-in block dropped at scene time `at`. Saved clips are
/// applied through the clip library instead and return an error here.
pub fn events(
    script: &Script,
    scene_index: usize,
    actor: &str,
    at: f64,
    block: &Block,
) -> Result<Vec<Event>, String> {
    if !script.characters.contains_key(actor) {
        return Err("Pudota palikka hahmon raidalle.".into());
    }
    let at = frame_snap(script, scene_index, at)?;
    let scene = &script.scenes[scene_index];
    let remaining = scene.duration - at;
    let rate = f64::from(script.project.fps);
    // Round down to whole frames so the event ends on or before the scene end.
    let length = |wanted: f64| ((wanted.min(remaining) * rate).floor() / rate).max(1.0 / rate);
    let actor = actor.to_owned();
    Ok(match block {
        Block::Gesture(value) => vec![Event::Gesture {
            at,
            actor,
            value: (*value).into(),
            duration: length(DEFAULT_SECONDS),
            principles: None,
        }],
        Block::Face(value) => vec![Event::Face {
            at,
            actor,
            value: (*value).into(),
        }],
        Block::Gait(gait) => {
            // Moves may not overlap: stop before this actor's next move starts.
            let mut end = scene.duration;
            for event in &scene.events {
                if let Event::Move {
                    at: start,
                    actor: a,
                    duration,
                    ..
                } = event
                {
                    if *a != actor {
                        continue;
                    }
                    if *start <= at && at < start + duration {
                        return Err(
                            "Hahmolla on jo liike tässä kohdassa. Pudota palikka liikkeen jälkeen."
                                .into(),
                        );
                    }
                    if *start > at {
                        end = end.min(*start);
                    }
                }
            }
            let duration = ((DEFAULT_SECONDS.min(end - at) * rate).floor()) / rate;
            if duration <= 0.0 {
                return Err("Seuraava liike alkaa liian pian. Pudota palikka aiemmin.".into());
            }
            let pose = motion_scene::evaluate(script, scene_index, at)
                .map_err(|e| e.to_string())?
                .actors
                .into_iter()
                .find(|p| p.id == actor)
                .ok_or("Hahmo on piilotettu tässä kohtauksessa.")?;
            let width = f64::from(script.project.width);
            let height = f64::from(script.project.height);
            let margin = width * 0.08;
            let direction = if pose.x <= width / 2.0 { 1.0 } else { -1.0 };
            let x =
                (pose.x + direction * speed(gait) * width * duration).clamp(margin, width - margin);
            let x = if (x - pose.x).abs() < 1.0 {
                // Already at the edge: head back across the stage instead.
                (pose.x - direction * speed(gait) * width * duration).clamp(margin, width - margin)
            } else {
                x
            };
            let y = pose.y.clamp(0.0, height);
            let mut events = Vec::new();
            // Walking backwards faces away from the travel direction.
            let facing = if (x > pose.x) != (*gait == "backwards") {
                "right"
            } else {
                "left"
            };
            if facing_at(&scene.events, &actor, at) != facing {
                events.push(Event::Face {
                    at,
                    actor: actor.clone(),
                    value: facing.into(),
                });
            }
            events.push(Event::Move {
                at,
                actor,
                x,
                y,
                duration,
                gait: (*gait != "walk").then(|| (*gait).into()),
                ease: None,
                pace: None,
            });
            events
        }
        Block::Clip(_) => return Err("Tallennettu liike lisätään liikekirjaston kautta.".into()),
    })
}

/// Insert block events at the end of the scene, validating the whole script
/// after each insertion. Returns the new source and the inserted indices.
pub fn insert(
    source: &str,
    scene_index: usize,
    first_index: usize,
    events: Vec<Event>,
) -> Result<(String, Vec<usize>), String> {
    let mut source = source.to_owned();
    let mut indices = Vec::new();
    for (offset, event) in events.into_iter().enumerate() {
        let index = first_index + offset;
        source = motion_script::insert_event(&source, scene_index, index, event)
            .map_err(|e| e.to_string())?;
        indices.push(index);
    }
    Ok((source, indices))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = "project: {title: T, width: 1000, height: 500, fps: 24}\ncharacters:\n  A:\n    asset: characters/a\nscenes:\n  - id: s\n    duration: 4\n    events:\n      - {type: move, actor: A, at: 2.5, x: 100, y: 350, duration: 1}\n";

    fn script(source: &str) -> Script {
        motion_script::parse(source).unwrap()
    }

    #[test]
    fn gait_block_turns_and_travels_toward_the_middle() {
        let script = script(SOURCE);
        // A starts at the center (500); the center counts as the left half.
        let events = events(&script, 0, "A", 0.0, &Block::Gait("run")).unwrap();
        match &events[..] {
            [Event::Face { value, at, .. }, Event::Move {
                x,
                duration,
                gait,
                at: start,
                ..
            }] => {
                assert_eq!((value.as_str(), *at, *start), ("right", 0.0, 0.0));
                assert_eq!(*duration, 1.5);
                assert!(*x > 500.0 && *x <= 920.0);
                assert_eq!(gait.as_deref(), Some("run"));
            }
            other => panic!("{other:?}"),
        }
        let (source, indices) = insert(SOURCE, 0, 1, events).unwrap();
        assert_eq!(indices, vec![1, 2]);
        assert!(motion_script::parse(&source).is_ok());
    }

    #[test]
    fn gait_block_stops_before_the_next_move_and_rejects_overlaps() {
        let script = script(SOURCE);
        let events = events(&script, 0, "A", 1.5, &Block::Gait("walk")).unwrap();
        let Some(Event::Move { duration, gait, .. }) = events.last() else {
            panic!("{events:?}");
        };
        assert_eq!(
            (*duration, gait.clone()),
            (1.0, None),
            "walk stays implicit"
        );
        assert!(super::events(&script, 0, "A", 3.0, &Block::Gait("jog")).is_err());
        // After the move A stands at x = 100 and faces nothing yet: heads right.
        let after = super::events(&script, 0, "A", 3.5, &Block::Gait("walk")).unwrap();
        assert!(matches!(&after[0], Event::Face { value, .. } if value == "right"));
    }

    #[test]
    fn gestures_and_facing_snap_and_fit_the_scene() {
        let script = script(SOURCE);
        let wave = events(&script, 0, "A", 3.51, &Block::Gesture("wave")).unwrap();
        match &wave[..] {
            [Event::Gesture { at, duration, .. }] => {
                assert_eq!(*at, 84.0 / 24.0);
                assert!(at + duration <= 4.0 && *duration > 0.0);
            }
            other => panic!("{other:?}"),
        }
        let back = events(&script, 0, "A", 10.0, &Block::Face("back")).unwrap();
        assert!(matches!(&back[..], [Event::Face { at, .. }] if *at == 95.0 / 24.0));
        assert!(events(&script, 0, "B", 0.0, &Block::Face("back")).is_err());
        assert!(events(&script, 0, "A", f64::NAN, &Block::Face("back")).is_err());
    }

    #[test]
    fn palette_lists_every_builtin_and_saved_clips() {
        let names: Vec<_> = builtin()
            .iter()
            .flat_map(|(_, blocks)| blocks.iter().map(Block::label))
            .collect();
        for expected in [
            "Kävely",
            "Hölkkä",
            "Juoksu",
            "Vilkutus",
            "Nyökkäys",
            "Selkä katsojaan",
        ] {
            assert!(names.iter().any(|n| n == expected), "{expected}");
        }
        assert!(saved(&script(SOURCE), Some("A")).is_empty());
    }
}
