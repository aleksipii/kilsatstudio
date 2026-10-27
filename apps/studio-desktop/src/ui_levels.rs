//! Progressive disclosure (A09): three interface levels and canvas-drag recording.
//!
//! The level is desktop UI state only. It is never written to the screenplay or the
//! `.kilsat` project document, and changing it never changes evaluation or export.
//!
//! [`DragRecorder`] turns canvas drags into EventLog v1 `drag` / `release` events
//! on the 24 kHz dialogue clock. The log is plain data: evaluation does not read it
//! yet. Converting a release into spring initial values is A04/A06 work (KS-003b,
//! KS-003d) and is not done here.
use eframe::egui;
use motion_script::event_log::{
    EventLog, EventSource, ExternalEvent, LoggedEvent, MAX_DRAG_COORDINATE, MAX_LOGGED_EVENTS,
};

/// How much of the editor is shown. Higher levels include everything below them.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum UiLevel {
    /// Level 1: screenplay editor, canvas, play/pause/scrub, speech and export.
    #[default]
    Script,
    /// Level 2: visual controls (viseme check, pose/expression keys, head, props, camera).
    Visual,
    /// Level 3: rules, rig joints and clips, technical checks.
    Deep,
}

/// Tool groups gated by level. Each maps to existing inspector sections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tool {
    /// Selected actor and event inspector.
    EventInspector,
    /// Mouth shape at the playhead from prepared scene audio.
    VisemeCheck,
    /// Pose/expression keys, head pose, hand props and camera framing.
    PoseControls,
    /// Read-only A06 spring profiles (choice waits for KS-003d).
    PhysicsProfile,
    /// Read-only rule list.
    RuleList,
    /// Custom rig, bones, joints, cue rules and clips.
    RigTools,
    /// Spring mass, stiffness and damping, read-only (A06).
    SpringParameters,
    /// Technical checks tab (alignment, GPU, metrics, runtime).
    AdvancedTab,
}

impl UiLevel {
    pub const ALL: [Self; 3] = [Self::Script, Self::Visual, Self::Deep];

    pub fn number(self) -> u8 {
        match self {
            Self::Script => 1,
            Self::Visual => 2,
            Self::Deep => 3,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Script => "1 · Käsikirjoitus",
            Self::Visual => "2 · Visuaalinen",
            Self::Deep => "3 · Syvä",
        }
    }
    pub fn summary(self) -> &'static str {
        match self {
            Self::Script => "Käsikirjoitus, esikatselu, toisto, puhe ja vienti.",
            Self::Visual => {
                "Lisäksi viseemitarkistus, asento- ja ilmeavaimet, pää, esineet ja kamera."
            }
            Self::Deep => "Lisäksi säännöt, rig-nivelet, klipit ja tekniset tarkistukset.",
        }
    }
    fn minimum(tool: Tool) -> Self {
        match tool {
            Tool::EventInspector => Self::Script,
            Tool::VisemeCheck | Tool::PoseControls | Tool::PhysicsProfile => Self::Visual,
            Tool::RuleList | Tool::RigTools | Tool::SpringParameters | Tool::AdvancedTab => {
                Self::Deep
            }
        }
    }
    pub fn shows(self, tool: Tool) -> bool {
        self >= Self::minimum(tool)
    }
}

/// Toolbar picker. Returns true when the level changed.
pub fn level_picker(ui: &mut egui::Ui, level: &mut UiLevel) -> bool {
    let before = *level;
    ui.label("Taso");
    for choice in UiLevel::ALL {
        ui.selectable_value(level, choice, choice.label())
            .on_hover_text(choice.summary());
    }
    before != *level
}

/// Most intermediate drag events recorded for one gesture; further motion only updates
/// the canvas. The final position on release is always recorded as well.
pub const MAX_DRAG_EVENTS_PER_GESTURE: usize = 64;
/// Smallest pointer movement in scene units that records a new drag event.
pub const MIN_DRAG_STEP: f64 = 1.0;

struct Active {
    scene: String,
    sample: u64,
    actor: String,
    joint: String,
    origin: [f64; 2],
    last: [f64; 2],
    recorded: usize,
}

/// Records canvas drags into an EventLog. Every started gesture ends with exactly one
/// `release` (pointer up or Escape), so a replayed log never leaves a joint held.
pub struct DragRecorder {
    log: EventLog,
    active: Option<Active>,
}

impl Default for DragRecorder {
    fn default() -> Self {
        Self::new(EventLog::new(motion_audio::clock::DIALOGUE_SAMPLE_RATE))
    }
}

impl DragRecorder {
    /// Continue an existing log (e.g. loaded from a project). Its sample rate must be
    /// the dialogue clock so drags line up with audio and mouth timing.
    pub fn new(log: EventLog) -> Self {
        Self { log, active: None }
    }
    pub fn from_project(log: Option<EventLog>) -> Result<Self, String> {
        match log {
            None => Ok(Self::default()),
            Some(log) if log.sample_rate == motion_audio::clock::DIALOGUE_SAMPLE_RATE => {
                Ok(Self::new(log))
            }
            Some(log) => Err(format!(
                "Tapahtumaloki käyttää {} Hz kelloa; vain {} Hz tuetaan",
                log.sample_rate,
                motion_audio::clock::DIALOGUE_SAMPLE_RATE
            )),
        }
    }
    pub fn log(&self) -> &EventLog {
        &self.log
    }
    /// The log for saving, or `None` when nothing was recorded.
    pub fn project_log(&self) -> Option<EventLog> {
        (!self.log.events.is_empty()).then(|| self.log.clone())
    }
    pub fn is_dragging(&self) -> bool {
        self.active.is_some()
    }
    pub fn clear(&mut self) {
        self.active = None;
        self.log.events.clear();
    }

    /// Start a gesture at scene-local `seconds`; `origin` is the joint's world position.
    pub fn begin(
        &mut self,
        scene: &str,
        seconds: f64,
        actor: &str,
        joint: &str,
        origin: [f64; 2],
    ) -> Result<(), String> {
        self.cancel();
        if self.log.events.len() + 2 > MAX_LOGGED_EVENTS {
            return Err(format!(
                "Tapahtumaloki on täynnä ({MAX_LOGGED_EVENTS} tapahtumaa); tyhjennä se tallentaaksesi raahauksia"
            ));
        }
        let sample = motion_audio::clock::seconds_to_sample(seconds).map_err(|e| e.to_string())?;
        let target = checked(origin)?;
        self.active = Some(Active {
            scene: scene.into(),
            sample,
            actor: actor.into(),
            joint: joint.into(),
            origin: target,
            last: target,
            recorded: 0,
        });
        self.push(EventSource::Pointer, |a| ExternalEvent::Drag {
            actor: a.actor.clone(),
            joint: a.joint.clone(),
            x: target[0],
            y: target[1],
        });
        Ok(())
    }

    /// Record `origin + delta` if it moved enough. Returns true when an event was added.
    pub fn update(&mut self, delta: [f64; 2]) -> bool {
        let room = self.log.events.len() + 2 <= MAX_LOGGED_EVENTS;
        let Some(active) = &mut self.active else {
            return false;
        };
        let Ok(target) = checked([active.origin[0] + delta[0], active.origin[1] + delta[1]]) else {
            return false;
        };
        let moved = (target[0] - active.last[0]).hypot(target[1] - active.last[1]);
        if moved < MIN_DRAG_STEP || active.recorded >= MAX_DRAG_EVENTS_PER_GESTURE || !room {
            return false;
        }
        active.last = target;
        self.push(EventSource::Pointer, |a| ExternalEvent::Drag {
            actor: a.actor.clone(),
            joint: a.joint.clone(),
            x: target[0],
            y: target[1],
        });
        true
    }

    /// Pointer released: record the final target (if moved) and the release.
    pub fn release(&mut self, delta: Option<[f64; 2]>) {
        if let Some(delta) = delta {
            if let Some(active) = &mut self.active {
                // The final position is always kept, even past the per-gesture cap.
                active.recorded = active.recorded.min(MAX_DRAG_EVENTS_PER_GESTURE - 1);
            }
            self.update(delta);
        }
        self.finish(EventSource::Pointer);
    }

    /// Escape or a lost gesture: release without a new target.
    pub fn cancel(&mut self) {
        self.finish(EventSource::Keyboard);
    }

    fn finish(&mut self, source: EventSource) {
        if self.active.is_some() {
            self.push(source, |a| ExternalEvent::Release {
                actor: a.actor.clone(),
                joint: a.joint.clone(),
            });
            self.active = None;
        }
    }

    /// Insert after every earlier-or-equal event of the same scene so per-scene
    /// sample order (EventLog v1) holds even when drags happen at earlier playheads.
    fn push(&mut self, source: EventSource, value: impl Fn(&Active) -> ExternalEvent) {
        let Some(active) = &mut self.active else {
            return;
        };
        let event = LoggedEvent {
            scene: active.scene.clone(),
            sample: active.sample,
            source,
            value: value(active),
        };
        if matches!(event.value, ExternalEvent::Drag { .. }) {
            active.recorded += 1;
        }
        let index = self
            .log
            .events
            .iter()
            .position(|e| e.scene == event.scene && e.sample > event.sample)
            .unwrap_or(self.log.events.len());
        self.log.events.insert(index, event);
    }
}

fn checked(world: [f64; 2]) -> Result<[f64; 2], String> {
    if world
        .iter()
        .all(|v| v.is_finite() && v.abs() <= MAX_DRAG_COORDINATE)
    {
        Ok(world)
    } else {
        Err("Raahauksen kohde on näyttämön rajojen ulkopuolella".into())
    }
}

/// The joint a whole-character canvas drag moves: `hip` for the built-in humanoid,
/// the parentless root joint for a custom rig.
pub fn drag_joint(script: &motion_script::Script, actor: &str) -> Option<String> {
    let character = script.characters.get(actor)?;
    match &character.rig {
        None => Some(motion_animation::HUMANOID_BONES[motion_animation::HIP].to_owned()),
        Some(rig) => rig
            .joints
            .iter()
            .find(|joint| joint.parent.is_none())
            .map(|joint| joint.id.clone()),
    }
}

/// One row of the read-only spring profile table (A06 `SpringProfile`).
pub fn spring_profile_row(profile: motion_animation::spring::SpringProfile) -> String {
    use motion_animation::spring::DampingRegime;
    let params = profile.params();
    let regime = match params.regime() {
        DampingRegime::Underdamped => "joustaa yli",
        DampingRegime::Critical => "asettuu ilman ylitystä",
        DampingRegime::Overdamped => "hidas, ei ylitystä",
    };
    let (m, k, c) = profile.coefficients();
    format!(
        "{}: m {m}, k {k}, c {c} · ζ {:.2} ({regime})",
        profile.name(),
        params.damping_ratio()
    )
}

/// Read-only list of A06 spring profiles. `coefficients` adds m, k and c (level 3).
pub fn spring_profiles_ui(ui: &mut egui::Ui, coefficients: bool) {
    use motion_animation::spring::SpringProfile;
    for profile in SpringProfile::ALL {
        if coefficients {
            ui.monospace(spring_profile_row(profile));
        } else {
            let zeta = profile.params().damping_ratio();
            ui.label(format!("{} · ζ {zeta:.2}", profile.name()));
        }
    }
    ui.weak(
        "Vain luettava. Profiilin valinta ja vapautuksen jousi kytketään, kun A04 ratkaisee \
         vapautuksen kohtausarvioinnissa (KS-003d). Raahaukset kirjataan jo tapahtumalokiin.",
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = "characters: {mia: {asset: mia.svg}}\nscenes: [{id: intro, duration: 4, events: []}, {id: outro, duration: 2, events: []}]\n";

    #[test]
    fn spring_rows_show_a06_coefficients_and_regime() {
        use motion_animation::spring::SpringProfile;
        assert_eq!(
            spring_profile_row(SpringProfile::Snappy),
            "snappy: m 1, k 400, c 40 · ζ 1.00 (asettuu ilman ylitystä)"
        );
        assert!(spring_profile_row(SpringProfile::Wobbly).contains("joustaa yli"));
    }

    #[test]
    fn levels_are_cumulative_and_default_to_script() {
        assert_eq!(UiLevel::default(), UiLevel::Script);
        let tools = [
            Tool::EventInspector,
            Tool::VisemeCheck,
            Tool::PoseControls,
            Tool::PhysicsProfile,
            Tool::RuleList,
            Tool::RigTools,
            Tool::SpringParameters,
            Tool::AdvancedTab,
        ];
        let count = |level: UiLevel| tools.iter().filter(|t| level.shows(**t)).count();
        assert_eq!(count(UiLevel::Script), 1);
        assert_eq!(count(UiLevel::Visual), 4);
        assert_eq!(count(UiLevel::Deep), tools.len());
        for tool in tools {
            if UiLevel::Script.shows(tool) {
                assert!(UiLevel::Visual.shows(tool));
            }
            if UiLevel::Visual.shows(tool) {
                assert!(UiLevel::Deep.shows(tool));
            }
        }
        assert_eq!(
            UiLevel::ALL.map(UiLevel::number),
            [1, 2, 3],
            "levels are numbered 1-3"
        );
    }

    #[test]
    fn a_gesture_records_drags_then_one_release_and_validates_against_the_script() {
        let script = motion_script::parse(SOURCE).unwrap();
        let joint = drag_joint(&script, "mia").unwrap();
        assert_eq!(joint, "hip");
        let mut recorder = DragRecorder::default();
        recorder
            .begin("intro", 1.0, "mia", &joint, [0.0, 0.0])
            .unwrap();
        assert!(!recorder.update([0.2, 0.0]), "below the step threshold");
        assert!(recorder.update([5.0, 0.0]));
        recorder.release(Some([9.0, 1.0]));
        let log = recorder.log();
        assert_eq!(log.sample_rate, 24_000);
        let kinds: Vec<_> = log.events.iter().map(|e| e.value.kind()).collect();
        assert_eq!(kinds, ["drag", "drag", "drag", "release"]);
        assert!(log.events.iter().all(|e| e.sample == 24_000));
        log.validate_for(&script).unwrap();
        // Round trip through the serialized v1 format.
        let reloaded = EventLog::from_json(&log.to_json().unwrap()).unwrap();
        assert_eq!(&reloaded, log);
    }

    #[test]
    fn cancelled_gestures_release_from_the_keyboard_and_earlier_drags_keep_scene_order() {
        let script = motion_script::parse(SOURCE).unwrap();
        let mut recorder = DragRecorder::default();
        recorder
            .begin("intro", 3.0, "mia", "hip", [0.0, 0.0])
            .unwrap();
        recorder.cancel();
        recorder
            .begin("outro", 0.5, "mia", "hip", [1.0, 1.0])
            .unwrap();
        recorder.release(None);
        // A later drag at an earlier playhead in the same scene.
        recorder
            .begin("intro", 1.0, "mia", "hip", [2.0, 2.0])
            .unwrap();
        recorder.release(None);
        let log = recorder.log();
        log.validate_for(&script).unwrap();
        let intro: Vec<_> = log.scene_events("intro").map(|(_, e)| e.sample).collect();
        assert_eq!(intro, [24_000, 24_000, 72_000, 72_000]);
        assert!(log.events.iter().any(|e| e.source == EventSource::Keyboard
            && matches!(e.value, ExternalEvent::Release { .. })));
        assert_eq!(recorder.project_log().unwrap().events.len(), 6);
        recorder.clear();
        assert!(recorder.project_log().is_none());
    }

    #[test]
    fn gestures_are_bounded_and_reject_invalid_input() {
        let mut recorder = DragRecorder::default();
        recorder
            .begin("intro", 0.0, "mia", "hip", [0.0, 0.0])
            .unwrap();
        for step in 1..500 {
            recorder.update([step as f64 * 2.0, 0.0]);
        }
        recorder.release(Some([5_000.0, 0.0]));
        let drags = recorder
            .log()
            .events
            .iter()
            .filter(|e| e.value.kind() == "drag")
            .count();
        assert_eq!(drags, MAX_DRAG_EVENTS_PER_GESTURE + 1);
        assert!(matches!(
            recorder.log().events.last().unwrap().value,
            ExternalEvent::Release { .. }
        ));
        assert!(recorder
            .begin("intro", f64::NAN, "mia", "hip", [0.0, 0.0])
            .is_err());
        assert!(recorder
            .begin("intro", 0.0, "mia", "hip", [f64::INFINITY, 0.0])
            .is_err());
        assert!(!recorder.is_dragging());
        let mut full = EventLog::new(24_000);
        full.events = recorder.log().events[..1]
            .iter()
            .cloned()
            .cycle()
            .take(MAX_LOGGED_EVENTS - 1)
            .collect();
        let mut recorder = DragRecorder::new(full);
        assert!(recorder
            .begin("intro", 0.0, "mia", "hip", [0.0, 0.0])
            .is_err());
        assert!(DragRecorder::from_project(Some(EventLog::new(48_000))).is_err());
    }
}
