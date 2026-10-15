//! Pure, frame-snapped timeline drag state. Document changes happen on release.
use motion_script::Event;

#[derive(Debug, Clone)]
pub struct Drag {
    pub scene_index: usize,
    pub event_index: usize,
    pub original_at: f64,
    pub proposed_at: f64,
    pointer_origin_x: f32,
    duration: f64,
}

impl Drag {
    pub fn new(
        scene_index: usize,
        event_index: usize,
        event: &Event,
        pointer_x: f32,
    ) -> Option<Self> {
        let original_at = event.actor_time().0;
        let duration = match event {
            Event::Move { duration, .. } | Event::Gesture { duration, .. } => *duration,
            _ => 0.0,
        };
        if !original_at.is_finite()
            || original_at < 0.0
            || !pointer_x.is_finite()
            || !duration.is_finite()
            || duration < 0.0
            || (matches!(event, Event::Move { .. } | Event::Gesture { .. }) && duration == 0.0)
        {
            return None;
        }
        Some(Self {
            scene_index,
            event_index,
            original_at,
            proposed_at: original_at,
            pointer_origin_x: pointer_x,
            duration,
        })
    }

    pub fn update(
        &mut self,
        pointer_x: f32,
        track_width: f32,
        scene_duration: f64,
        fps: u32,
    ) -> Result<(), String> {
        if !pointer_x.is_finite()
            || !track_width.is_finite()
            || track_width <= 0.0
            || !scene_duration.is_finite()
            || scene_duration <= 0.0
            || fps == 0
            || self.duration > scene_duration
        {
            return Err("Timeline drag needs finite coordinates, positive width, scene duration and frame rate, and an event fitting the scene".into());
        }
        let rate = f64::from(fps);
        let limit = (scene_duration - self.duration) * rate;
        if !limit.is_finite() || limit >= (1_u64 << 53) as f64 {
            return Err("Scene is too long for accurate frame snapping".into());
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
            return Err("Event does not fit on the scene frame grid".into());
        }
        let delta = (f64::from(pointer_x) - f64::from(self.pointer_origin_x))
            / f64::from(track_width)
            * scene_duration;
        // A click without movement must not quantize an existing off-grid start.
        self.proposed_at = if delta == 0.0 {
            self.original_at
        } else {
            ((self.original_at + delta) * rate)
                .round()
                .clamp(0.0, last_frame)
                / rate
        };
        Ok(())
    }

    pub fn replacement(&self, event: &Event) -> Event {
        let mut replacement = event.clone();
        match &mut replacement {
            Event::Move { at, .. }
            | Event::Gesture { at, .. }
            | Event::Speak { at, .. }
            | Event::Emotion { at, .. } => *at = self.proposed_at,
        }
        replacement
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
        assert_eq!(drag.original_at, 0.123);
        match drag.replacement(&event) {
            Event::Speak {
                at,
                text,
                emotion,
                audio,
                actor,
            } => {
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
        };
        assert!(Drag::new(0, 0, &event, 0.0).is_none());
    }
}
