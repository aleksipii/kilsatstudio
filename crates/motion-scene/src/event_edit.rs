//! Frame-snapped event retiming and new event drafts, shared by every editor.
//!
//! The Mac timeline (mouse) and the iPad timeline (touch) both move events with
//! [`Retime`] and add them with [`create`], so a drag lands on the same frame
//! everywhere. Nothing here changes a document: callers apply the returned
//! event with `motion_script::apply_event` or `insert_event`, which validate the
//! whole screenplay.
use motion_script::{Event, Script};

/// Why an event edit was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{0}")]
pub struct EventEditError(String);

impl EventEditError {
    fn new(message: &str) -> Self {
        Self(message.to_owned())
    }
}

/// Moves an event's start, or its end edge when resizing, on the scene frame grid.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Retime {
    /// Start time before the edit, in seconds.
    pub original_at: f64,
    /// Length before the edit (0 for point events), in seconds.
    pub duration: f64,
    /// True when the end edge moves (length) rather than the whole event.
    pub resize: bool,
}

impl Retime {
    /// Move the whole event. `None` for a non-finite or negative start or length.
    pub fn new(event: &Event) -> Option<Self> {
        let original_at = event.actor_time().0;
        let duration = length(event).unwrap_or(0.0);
        if !original_at.is_finite()
            || original_at < 0.0
            || !duration.is_finite()
            || duration < 0.0
            || (length(event).is_some() && duration == 0.0)
        {
            return None;
        }
        Some(Self {
            original_at,
            duration,
            resize: false,
        })
    }

    /// Move the end edge of an event with a length (move or gesture).
    pub fn resize(event: &Event) -> Option<Self> {
        length(event)?;
        Some(Self {
            resize: true,
            ..Self::new(event)?
        })
    }

    /// The start and length after shifting by `delta` seconds, snapped to whole
    /// frames and kept inside the scene. A zero delta keeps an off-grid start.
    pub fn snap(
        &self,
        delta: f64,
        scene_duration: f64,
        fps: u32,
    ) -> Result<(f64, f64), EventEditError> {
        if !delta.is_finite()
            || !scene_duration.is_finite()
            || scene_duration <= 0.0
            || fps == 0
            || self.duration > scene_duration
        {
            return Err(EventEditError::new(
                "Timeline edit needs a finite shift, scene duration and frame rate, and an event fitting the scene",
            ));
        }
        let rate = f64::from(fps);
        if self.resize {
            return self
                .snap_end(delta, scene_duration, rate)
                .map(|length| (self.original_at, length));
        }
        let limit = (scene_duration - self.duration) * rate;
        if !limit.is_finite() || limit >= (1_u64 << 53) as f64 {
            return Err(EventEditError::new(
                "Scene is too long for accurate frame snapping",
            ));
        }
        let valid = |frame: f64| {
            let at = frame / rate;
            at < scene_duration && at + self.duration <= scene_duration
        };
        let mut last_frame = limit.floor();
        // Division and addition determine validity, as in MotionScript validation.
        // Correct multiplication rounding at exact or nearly exact boundaries.
        if valid(last_frame + 1.0) {
            last_frame += 1.0;
        }
        if !valid(last_frame) {
            last_frame -= 1.0;
        }
        if last_frame < 0.0 {
            return Err(EventEditError::new(
                "Event does not fit on the scene frame grid",
            ));
        }
        let at = if delta == 0.0 {
            self.original_at
        } else {
            ((self.original_at + delta) * rate)
                .round()
                .clamp(0.0, last_frame)
                / rate
        };
        Ok((at, self.duration))
    }

    /// The end snaps to the frame grid, at least one frame after the start and
    /// never past the scene end.
    fn snap_end(&self, delta: f64, scene_duration: f64, rate: f64) -> Result<f64, EventEditError> {
        if delta == 0.0 {
            return Ok(self.duration);
        }
        let first = (self.original_at * rate).floor() + 1.0;
        let last = (scene_duration * rate).floor();
        if !last.is_finite() || last >= (1_u64 << 53) as f64 || first > last {
            return Err(EventEditError::new(
                "Event does not fit on the scene frame grid",
            ));
        }
        let end = ((self.original_at + self.duration + delta) * rate)
            .round()
            .clamp(first, last)
            / rate;
        let duration = end - self.original_at;
        if !(duration > 0.0 && self.original_at + duration <= scene_duration) {
            return Err(EventEditError::new(
                "Event length must stay positive and inside the scene",
            ));
        }
        Ok(duration)
    }

    /// `event` with start `at`; with `resize`, also length `duration`. Every
    /// other field is kept.
    pub fn apply(&self, event: &Event, at: f64, duration: f64) -> Event {
        let mut replacement = event.clone();
        *start(&mut replacement) = at;
        if self.resize {
            if let Event::Move { duration: d, .. } | Event::Gesture { duration: d, .. } =
                &mut replacement
            {
                *d = duration;
            }
        }
        replacement
    }
}

/// Length of a move or gesture; `None` for point events.
pub fn length(event: &Event) -> Option<f64> {
    match event {
        Event::Move { duration, .. } | Event::Gesture { duration, .. } => Some(*duration),
        _ => None,
    }
}

fn start(event: &mut Event) -> &mut f64 {
    match event {
        Event::Move { at, .. }
        | Event::Emotion { at, .. }
        | Event::Gesture { at, .. }
        | Event::Speak { at, .. }
        | Event::Face { at, .. }
        | Event::Hold { at, .. }
        | Event::Pose { at, .. } => at,
    }
}

/// Kind of event a new draft starts as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Move,
    Emotion,
    Gesture,
    Speak,
    Face,
}

fn snapped_time(script: &Script, scene_index: usize, at: f64) -> Result<f64, EventEditError> {
    if !at.is_finite() {
        return Err(EventEditError::new("Event time must be finite"));
    }
    let scene = script
        .scenes
        .get(scene_index)
        .ok_or_else(|| EventEditError::new("Select an existing scene"))?;
    let count = crate::frame_count(scene.duration, script.project.fps, (1_u64 << 53) - 1)
        .map_err(|error| EventEditError(error.to_string()))?;
    let rate = f64::from(script.project.fps);
    Ok((at * rate).round().clamp(0.0, (count - 1) as f64) / rate)
}

/// A new event at the nearest scene frame, strictly before the scene end.
/// Speech uses the character's voice configuration when applied; no audio is
/// fabricated.
pub fn create(
    script: &Script,
    scene_index: usize,
    actor: &str,
    at: f64,
    kind: Kind,
) -> Result<Event, EventEditError> {
    if !script.characters.contains_key(actor) {
        return Err(EventEditError::new("Select an existing character"));
    }
    let at = snapped_time(script, scene_index, at)?;
    let actor = actor.to_owned();
    let duration = (script.scenes[scene_index].duration - at).min(1.0);
    Ok(match kind {
        Kind::Move => {
            let scene = crate::evaluate(script, scene_index, at)
                .map_err(|error| EventEditError(error.to_string()))?;
            let pose = scene
                .actors
                .iter()
                .find(|pose| pose.id == actor)
                .ok_or_else(|| EventEditError::new("Character pose is unavailable"))?;
            Event::Move {
                at,
                actor,
                x: pose.x,
                y: pose.y,
                duration,
                gait: None,
                ease: None,
                pace: None,
                arc: None,
                bezier: None,
            }
        }
        Kind::Emotion => Event::Emotion {
            at,
            actor,
            value: "neutral".into(),
            intensity: None,
        },
        Kind::Gesture => Event::Gesture {
            at,
            actor,
            value: "wave".into(),
            duration,
            principles: None,
        },
        Kind::Face => Event::Face {
            at,
            actor,
            value: "front".into(),
        },
        Kind::Speak => Event::Speak {
            at,
            actor,
            text: "New dialogue.".into(),
            emotion: None,
            audio: None,
            intensity: None,
        },
    })
}

/// Clone every field except the frame-snapped start time. A length may extend
/// past the scene end: applying it is then refused until fixed.
pub fn duplicate_at(
    script: &Script,
    scene_index: usize,
    event_index: usize,
    at: f64,
) -> Result<Event, EventEditError> {
    let at = snapped_time(script, scene_index, at)?;
    let mut event = script.scenes[scene_index]
        .events
        .get(event_index)
        .ok_or_else(|| EventEditError::new("Select an existing event"))?
        .clone();
    *start(&mut event) = at;
    Ok(event)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn script() -> Script {
        motion_script::parse(include_str!("../../../examples/dialogue.yaml")).unwrap()
    }

    fn gesture(at: f64, duration: f64) -> Event {
        Event::Gesture {
            at,
            actor: "EMMA".into(),
            value: "wave".into(),
            duration,
            principles: None,
        }
    }

    #[test]
    fn moves_snap_to_frames_and_stay_inside_the_scene() {
        let retime = Retime::new(&gesture(0.5, 1.0)).unwrap();
        assert_eq!(retime.snap(0.26, 4.0, 24).unwrap(), (18.0 / 24.0, 1.0));
        assert_eq!(retime.snap(-9.0, 4.0, 24).unwrap(), (0.0, 1.0));
        assert_eq!(retime.snap(9.0, 4.0, 24).unwrap(), (3.0, 1.0));
        let off_grid = Retime::new(&gesture(0.123, 1.0)).unwrap();
        assert_eq!(off_grid.snap(0.0, 4.0, 24).unwrap().0, 0.123);
    }

    #[test]
    fn resizing_keeps_the_start_and_at_least_one_frame() {
        let event = gesture(0.5, 1.0);
        let retime = Retime::resize(&event).unwrap();
        assert_eq!(retime.snap(0.4, 4.0, 24).unwrap(), (0.5, 46.0 / 24.0 - 0.5));
        assert_eq!(retime.snap(-9.0, 4.0, 24).unwrap().1, 13.0 / 24.0 - 0.5);
        assert_eq!(retime.snap(9.0, 4.0, 24).unwrap().1, 3.5);
        match retime.apply(&event, 0.5, 3.5) {
            Event::Gesture {
                at,
                duration,
                value,
                ..
            } => assert_eq!((at, duration, value.as_str()), (0.5, 3.5, "wave")),
            _ => panic!(),
        }
        let face = Event::Face {
            at: 0.5,
            actor: "EMMA".into(),
            value: "left".into(),
        };
        assert!(Retime::resize(&face).is_none(), "no length to resize");
        assert_eq!(Retime::new(&face).unwrap().duration, 0.0);
    }

    #[test]
    fn invalid_inputs_are_refused() {
        assert!(Retime::new(&gesture(f64::NAN, 1.0)).is_none());
        assert!(Retime::new(&gesture(0.0, -1.0)).is_none());
        assert!(Retime::new(&gesture(0.0, 0.0)).is_none());
        let retime = Retime::new(&gesture(0.0, 1.0)).unwrap();
        for (delta, duration, fps) in [
            (f64::NAN, 4.0, 24),
            (0.1, f64::INFINITY, 24),
            (0.1, 0.0, 24),
            (0.1, 4.0, 0),
            (0.1, 0.5, 24),
        ] {
            assert!(retime.snap(delta, duration, fps).is_err());
        }
    }

    #[test]
    fn creates_bounded_defaults_without_touching_source() {
        let script = script();
        let before = format!("{script:?}");
        for kind in [
            Kind::Move,
            Kind::Emotion,
            Kind::Gesture,
            Kind::Speak,
            Kind::Face,
        ] {
            let event = create(&script, 0, "EMMA", 100.0, kind).unwrap();
            assert_eq!(event.actor_time(), (287.0 / 24.0, "EMMA"));
            match event {
                Event::Move { duration, x, y, .. } => {
                    assert!(duration > 0.0 && duration <= 1.0);
                    assert!(287.0 / 24.0 + duration <= 12.0);
                    assert_eq!((x, y), (960.0, 756.0));
                }
                Event::Gesture {
                    duration, value, ..
                } => {
                    assert!(duration > 0.0 && duration <= 1.0);
                    assert_eq!(value, "wave");
                }
                Event::Emotion { value, .. } => assert_eq!(value, "neutral"),
                Event::Face { value, .. } => assert_eq!(value, "front"),
                Event::Hold { .. } | Event::Pose { .. } => unreachable!("not created here"),
                Event::Speak { text, audio, .. } => {
                    assert_eq!(text, "New dialogue.");
                    assert!(audio.is_none());
                }
            }
        }
        assert_eq!(format!("{script:?}"), before);
        assert_eq!(
            create(&script, 0, "EMMA", -2.0, Kind::Speak)
                .unwrap()
                .actor_time()
                .0,
            0.0
        );
        assert_eq!(
            create(&script, 0, "EMMA", 0.07, Kind::Speak)
                .unwrap()
                .actor_time()
                .0,
            2.0 / 24.0
        );
    }

    #[test]
    fn validates_draft_inputs() {
        let script = script();
        assert!(create(&script, 0, "MISSING", 0.0, Kind::Speak).is_err());
        assert!(create(&script, 8, "EMMA", 0.0, Kind::Speak).is_err());
        for at in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(create(&script, 0, "EMMA", at, Kind::Speak).is_err());
            assert!(duplicate_at(&script, 0, 0, at).is_err());
        }
        assert!(duplicate_at(&script, 0, 100, 1.0).is_err());
    }

    #[test]
    fn fractional_and_subframe_scenes_keep_positive_durations() {
        let mut script = script();
        script.scenes[0].events.clear();
        for end in [1.01, 0.001, 1.0 / 24.0] {
            script.scenes[0].duration = end;
            match create(&script, 0, "EMMA", end, Kind::Gesture).unwrap() {
                Event::Gesture { at, duration, .. } => {
                    assert!(at < end);
                    assert!(duration > 0.0);
                    assert!(at + duration <= end);
                }
                _ => panic!("expected gesture"),
            }
        }
        script.project.fps = 0;
        assert!(create(&script, 0, "EMMA", 0.0, Kind::Speak).is_err());
    }

    #[test]
    fn duplication_preserves_all_fields_and_source() {
        let mut script = script();
        if let Event::Speak {
            audio, intensity, ..
        } = &mut script.scenes[0].events[0]
        {
            *audio = Some("audio/original.wav".into());
            *intensity = Some(0.4);
        }
        let before = format!("{:?}", script.scenes[0].events[0]);
        let event = duplicate_at(&script, 0, 0, 3.0).unwrap();
        match event {
            Event::Speak {
                at,
                actor,
                text,
                emotion,
                audio,
                intensity,
            } => {
                assert_eq!(intensity, Some(0.4));
                assert_eq!(at, 3.0);
                assert_eq!(actor, "EMMA");
                assert_eq!(text, "Hello Alex! Welcome to Kilsat Studio.");
                assert_eq!(emotion.as_deref(), Some("happy"));
                assert_eq!(audio.as_deref(), Some("audio/original.wav"));
            }
            _ => panic!("expected speech"),
        }
        assert_eq!(format!("{:?}", script.scenes[0].events[0]), before);
    }

    #[test]
    fn move_uses_evaluated_pose_and_duplicate_keeps_duration() {
        let mut script = script();
        script.scenes[0].events.push(Event::Move {
            at: 0.0,
            actor: "EMMA".into(),
            x: 100.0,
            y: 200.0,
            duration: 2.0,
            gait: None,
            ease: None,
            pace: None,
            arc: None,
            bezier: None,
        });
        let event = create(&script, 0, "EMMA", 1.0, Kind::Move).unwrap();
        match event {
            Event::Move { x, y, duration, .. } => assert_eq!((x, y, duration), (530.0, 478.0, 1.0)),
            _ => panic!("expected move"),
        }
        match duplicate_at(&script, 0, 2, 12.0).unwrap() {
            Event::Move { at, duration, .. } => {
                assert_eq!(duration, 2.0);
                assert!(at + duration > 12.0);
            }
            _ => panic!("expected move"),
        }
    }
}
