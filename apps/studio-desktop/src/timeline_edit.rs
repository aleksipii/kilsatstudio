//! Pure, frame-snapped timeline drag state. Document changes happen on release.
use motion_scene::event_edit::Retime;
use motion_script::Event;

#[derive(Debug, Clone)]
pub struct Drag {
    pub scene_index: usize,
    pub event_index: usize,
    pub proposed_at: f64,
    /// Length on release; changes only when resizing from the end edge.
    pub proposed_duration: f64,
    /// True when dragging the end edge (length) rather than the whole event.
    pub resize: bool,
    pointer_origin_x: f32,
    /// Shared frame snapping, the same as the iPad timeline.
    retime: Retime,
}

impl Drag {
    pub fn new(
        scene_index: usize,
        event_index: usize,
        event: &Event,
        pointer_x: f32,
    ) -> Option<Self> {
        Self::with(scene_index, event_index, Retime::new(event)?, pointer_x)
    }

    /// Drag the end edge of an event with a duration (move or gesture).
    pub fn resize(
        scene_index: usize,
        event_index: usize,
        event: &Event,
        pointer_x: f32,
    ) -> Option<Self> {
        Self::with(scene_index, event_index, Retime::resize(event)?, pointer_x)
    }

    fn with(
        scene_index: usize,
        event_index: usize,
        retime: Retime,
        pointer_x: f32,
    ) -> Option<Self> {
        if !pointer_x.is_finite() {
            return None;
        }
        Some(Self {
            scene_index,
            event_index,
            proposed_at: retime.original_at,
            proposed_duration: retime.duration,
            resize: retime.resize,
            pointer_origin_x: pointer_x,
            retime,
        })
    }

    pub fn update(
        &mut self,
        pointer_x: f32,
        track_width: f32,
        scene_duration: f64,
        fps: u32,
    ) -> Result<(), String> {
        if !pointer_x.is_finite() || !track_width.is_finite() || track_width <= 0.0 {
            return Err("Timeline drag needs finite coordinates, positive width, scene duration and frame rate, and an event fitting the scene".into());
        }
        let delta = (f64::from(pointer_x) - f64::from(self.pointer_origin_x))
            / f64::from(track_width)
            * scene_duration;
        let (at, duration) = self
            .retime
            .snap(delta, scene_duration, fps)
            .map_err(|error| error.to_string())?;
        self.proposed_at = at;
        self.proposed_duration = duration;
        Ok(())
    }

    pub fn replacement(&self, event: &Event) -> Event {
        self.retime
            .apply(event, self.proposed_at, self.proposed_duration)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn speech(at: f64) -> Event {
        Event::Speak {
            at,
            actor: "EMMA".into(),
            text: "Hello!".into(),
            emotion: Some("happy".into()),
            audio: Some("speech.wav".into()),
            intensity: None,
        }
    }
    #[test]
    fn clamps_and_snaps_point_events() {
        let mut drag = Drag::new(2, 3, &speech(0.5), 100.0).unwrap();
        drag.update(-100.0, 100.0, 1.0, 24).unwrap();
        assert_eq!(drag.proposed_at, 0.0);
        drag.update(1000.0, 100.0, 1.0, 24).unwrap();
        assert_eq!(drag.proposed_at, 23.0 / 24.0);
        drag.update(1000.0, 100.0, 1.01, 24).unwrap();
        assert_eq!(drag.proposed_at, 1.0);
        drag.update(1000.0, 100.0, 0.001, 24).unwrap();
        assert_eq!(drag.proposed_at, 0.0);
    }
    #[test]
    fn durations_fit_non_grid_scene_end() {
        let event = Event::Move {
            at: 0.0,
            actor: "EMMA".into(),
            x: 2.0,
            y: 3.0,
            duration: 0.3,
            gait: None,
            ease: None,
            pace: None,
            arc: None,
            bezier: None,
        };
        let mut drag = Drag::new(0, 0, &event, 0.0).unwrap();
        drag.update(1000.0, 100.0, 1.01, 24).unwrap();
        assert_eq!(drag.proposed_at, 17.0 / 24.0);
        assert!(drag.proposed_at + 0.3 <= 1.01);
        match drag.replacement(&event) {
            Event::Move { x, y, duration, .. } => assert_eq!((x, y, duration), (2.0, 3.0, 0.3)),
            _ => panic!(),
        }
    }
    #[test]
    fn click_preserves_off_grid_and_speech_fields() {
        let event = speech(0.123);
        let mut drag = Drag::new(2, 3, &event, 10.0).unwrap();
        drag.update(10.0, 100.0, 2.0, 24).unwrap();
        assert_eq!(drag.retime.original_at, 0.123);
        match drag.replacement(&event) {
            Event::Speak {
                at,
                text,
                emotion,
                audio,
                actor,
                intensity,
            } => {
                assert_eq!(intensity, None);
                assert_eq!(at, 0.123);
                assert_eq!(text, "Hello!");
                assert_eq!(actor, "EMMA");
                assert_eq!(emotion.as_deref(), Some("happy"));
                assert_eq!(audio.as_deref(), Some("speech.wav"));
            }
            _ => panic!(),
        }
    }
    #[test]
    fn resizing_snaps_the_end_and_keeps_the_start() {
        let event = Event::Gesture {
            at: 0.5,
            actor: "EMMA".into(),
            value: "wave".into(),
            duration: 1.0,
            principles: None,
        };
        assert!(
            Drag::resize(0, 0, &speech(0.5), 0.0).is_none(),
            "no length to resize"
        );
        let mut drag = Drag::resize(0, 0, &event, 0.0).unwrap();
        // 10 % of a 4 s track is +0.4 s; the end 1.9 s snaps to frame 46.
        drag.update(10.0, 100.0, 4.0, 24).unwrap();
        assert_eq!(drag.proposed_at, 0.5);
        assert_eq!(drag.proposed_duration, 46.0 / 24.0 - 0.5);
        drag.update(-1000.0, 100.0, 4.0, 24).unwrap();
        assert_eq!(
            drag.proposed_duration,
            13.0 / 24.0 - 0.5,
            "at least one frame"
        );
        drag.update(1000.0, 100.0, 4.0, 24).unwrap();
        assert_eq!(
            drag.proposed_at + drag.proposed_duration,
            4.0,
            "ends at the scene end"
        );
        match drag.replacement(&event) {
            Event::Gesture { at, duration, .. } => assert_eq!((at, duration), (0.5, 3.5)),
            _ => panic!(),
        }
    }
    #[test]
    fn rejects_invalid_inputs_without_mutating() {
        assert!(Drag::new(0, 0, &speech(f64::NAN), 0.0).is_none());
        assert!(Drag::new(0, 0, &speech(0.0), f32::INFINITY).is_none());
        let mut drag = Drag::new(0, 0, &speech(0.1), 0.0).unwrap();
        for (x, width, duration, fps) in [
            (f32::NAN, 100.0, 1.0, 24),
            (1.0, 0.0, 1.0, 24),
            (1.0, 100.0, f64::INFINITY, 24),
            (1.0, 100.0, 1.0, 0),
        ] {
            assert!(drag.update(x, width, duration, fps).is_err());
            assert_eq!(drag.proposed_at, 0.1);
        }
        let event = Event::Gesture {
            at: 0.0,
            actor: "EMMA".into(),
            value: "wave".into(),
            duration: -1.0,
            principles: None,
        };
        assert!(Drag::new(0, 0, &event, 0.0).is_none());
    }
}
