//! Timeline editing for the iPad: a summary of a scene's events, and edits that
//! return the new MotionScript source.
//!
//! Every edit starts from the document's last valid source, snaps to frames
//! with the shared `motion_scene::event_edit` (the same as the Mac timeline)
//! and is checked as a whole screenplay by `motion_script`. Nothing is written:
//! the app shows the returned source as its unsaved draft.
use super::{guarded, parse_source, Document};
use motion_scene::event_edit::{self, Kind, Retime};
use motion_script::Event;
use serde::Serialize;
use std::{
    ffi::{c_char, CStr, CString},
    ptr,
};

/// Largest screenplay an edit may produce; the Mac editor uses the same bound.
pub const MAX_EDITED_SOURCE_BYTES: usize = 256 * 1024;

/// One scene's tracks and events, sent to the app as JSON.
#[derive(Debug, Serialize)]
pub struct TimelineSummary {
    pub duration: f64,
    pub fps: u32,
    /// Characters in screenplay order, one track each.
    pub actors: Vec<String>,
    pub events: Vec<TimelineEvent>,
    /// Accepted values for each event kind's `value`.
    pub choices: Choices,
}

#[derive(Debug, Serialize)]
pub struct TimelineEvent {
    /// Index in the scene's event list, used by the edit calls.
    pub index: usize,
    pub actor: String,
    /// move, gesture, emotion, face, pose, hold or speak.
    pub kind: &'static str,
    /// Gesture, expression, facing or pose name; gait of a move; item held
    /// (empty when putting down); the line of a speech.
    pub value: String,
    pub at: f64,
    /// Length of a move or gesture; `None` for point events. A line's length
    /// comes from its audio, which is unknown until audio is prepared.
    pub duration: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct Choices {
    pub gesture: Vec<&'static str>,
    pub emotion: Vec<&'static str>,
    pub face: Vec<&'static str>,
    pub pose: Vec<&'static str>,
    pub gait: Vec<&'static str>,
}

fn summary_event(index: usize, event: &Event) -> TimelineEvent {
    let (at, actor) = event.actor_time();
    let (kind, value) = match event {
        Event::Move { gait, .. } => ("move", gait.clone().unwrap_or_else(|| "walk".into())),
        Event::Gesture { value, .. } => ("gesture", value.clone()),
        Event::Emotion { value, .. } => ("emotion", value.clone()),
        Event::Face { value, .. } => ("face", value.clone()),
        Event::Pose { value, .. } => ("pose", value.clone()),
        Event::Hold { item, .. } => ("hold", item.clone().unwrap_or_default()),
        Event::Speak { text, .. } => ("speak", text.clone()),
    };
    TimelineEvent {
        index,
        actor: actor.to_owned(),
        kind,
        value,
        at,
        duration: event_edit::length(event),
    }
}

/// Parse an app-side kind name.
pub fn kind(name: &str) -> Result<Kind, String> {
    Ok(match name {
        "move" => Kind::Move,
        "gesture" => Kind::Gesture,
        "emotion" => Kind::Emotion,
        "face" => Kind::Face,
        "speak" => Kind::Speak,
        other => return Err(format!("Tapahtumalajia {other:?} ei voi lisätä.")),
    })
}

impl Document {
    /// Tracks and events of `scene`.
    pub fn timeline(&self, scene: usize) -> Result<TimelineSummary, String> {
        let current = self.script.scenes.get(scene).ok_or("Kohtausta ei ole.")?;
        Ok(TimelineSummary {
            duration: current.duration,
            fps: self.script.project.fps,
            actors: self.script.characters.keys().cloned().collect(),
            events: current
                .events
                .iter()
                .enumerate()
                .map(|(index, event)| summary_event(index, event))
                .collect(),
            choices: Choices {
                gesture: motion_scene::GESTURES.to_vec(),
                emotion: motion_script::story::EMOTIONS.to_vec(),
                face: motion_script::FACINGS.to_vec(),
                pose: motion_script::POSES.to_vec(),
                gait: motion_scene::GAITS.to_vec(),
            },
        })
    }

    fn event(&self, scene: usize, event: usize) -> Result<&Event, String> {
        self.script
            .scenes
            .get(scene)
            .and_then(|s| s.events.get(event))
            .ok_or_else(|| "Tapahtumaa ei enää ole.".into())
    }

    fn retime(&self, scene: usize, event: usize, resize: bool) -> Result<Retime, String> {
        let event = self.event(scene, event)?;
        let retime = if resize {
            Retime::resize(event)
        } else {
            Retime::new(event)
        };
        retime.ok_or_else(|| {
            if resize {
                "Tällä tapahtumalla ei ole pituutta.".into()
            } else {
                "Tapahtuman aika ei kelpaa siirrettäväksi.".into()
            }
        })
    }

    /// Start and length after shifting the start (or with `resize`, the end) by
    /// `delta` seconds, snapped to frames inside the scene. For drag feedback.
    pub fn snap_event(
        &self,
        scene: usize,
        event: usize,
        delta: f64,
        resize: bool,
    ) -> Result<(f64, f64), String> {
        let duration = self
            .script
            .scenes
            .get(scene)
            .ok_or("Kohtausta ei ole.")?
            .duration;
        self.retime(scene, event, resize)?
            .snap(delta, duration, self.script.project.fps)
            .map_err(|e| e.to_string())
    }

    /// Source with the event shifted as [`Self::snap_event`] shows.
    pub fn move_event(
        &self,
        scene: usize,
        event: usize,
        delta: f64,
        resize: bool,
    ) -> Result<String, String> {
        let (at, duration) = self.snap_event(scene, event, delta, resize)?;
        let replacement =
            self.retime(scene, event, resize)?
                .apply(self.event(scene, event)?, at, duration);
        edited(motion_script::apply_event(
            &self.source,
            scene,
            event,
            replacement,
        ))
    }

    /// Source with a new event of `kind` for `actor` at the frame nearest `at`,
    /// appended to the scene.
    pub fn insert_event(
        &self,
        scene: usize,
        actor: &str,
        at: f64,
        kind: Kind,
    ) -> Result<String, String> {
        let event =
            event_edit::create(&self.script, scene, actor, at, kind).map_err(|e| e.to_string())?;
        let end = self.script.scenes[scene].events.len();
        edited(motion_script::insert_event(&self.source, scene, end, event))
    }

    /// Source with a copy of the event at the frame nearest `at`.
    pub fn duplicate_event(&self, scene: usize, event: usize, at: f64) -> Result<String, String> {
        let copy =
            event_edit::duplicate_at(&self.script, scene, event, at).map_err(|e| e.to_string())?;
        let end = self.script.scenes[scene].events.len();
        edited(motion_script::insert_event(&self.source, scene, end, copy))
    }

    /// Source without the event.
    pub fn remove_event(&self, scene: usize, event: usize) -> Result<String, String> {
        self.event(scene, event)?;
        edited(motion_script::remove_event(&self.source, scene, event))
    }

    /// Source with the event's `value` changed (see [`TimelineEvent::value`]).
    pub fn set_event_value(
        &self,
        scene: usize,
        event: usize,
        value: &str,
    ) -> Result<String, String> {
        let mut replacement = self.event(scene, event)?.clone();
        let value = value.trim().to_owned();
        match &mut replacement {
            Event::Gesture { value: v, .. }
            | Event::Emotion { value: v, .. }
            | Event::Face { value: v, .. }
            | Event::Pose { value: v, .. } => *v = value,
            Event::Move { gait, .. } => *gait = Some(value),
            Event::Hold { item, .. } => *item = (!value.is_empty()).then_some(value),
            Event::Speak { text, .. } => {
                if value.is_empty() {
                    return Err("Repliikki ei voi olla tyhjä.".into());
                }
                *text = value;
            }
        }
        edited(motion_script::apply_event(
            &self.source,
            scene,
            event,
            replacement,
        ))
    }
}

/// An edit's source, refused when too large or no longer a valid document.
fn edited(result: Result<String, motion_script::Error>) -> Result<String, String> {
    let source = result.map_err(|e| format!("Muutosta ei voitu tehdä: {e}"))?;
    if source.len() > MAX_EDITED_SOURCE_BYTES {
        return Err("Muokattu käsikirjoitus ylittää 256 KiB:n kokorajan.".into());
    }
    parse_source(&source)?;
    Ok(source)
}

/// A string for the app; free with `koeta_string_free`.
fn owned(text: String) -> Result<*mut c_char, String> {
    CString::new(text)
        .map(CString::into_raw)
        .map_err(|_| "Tekstissä on NUL-merkki.".into())
}

/// # Safety
/// `text` must be null or NUL-terminated.
unsafe fn c_text<'a>(text: *const c_char) -> Result<&'a str, String> {
    if text.is_null() {
        return Err("Teksti puuttuu.".into());
    }
    // SAFETY: the caller passes a NUL-terminated string.
    unsafe { CStr::from_ptr(text) }
        .to_str()
        .map_err(|_| "Teksti ei ole UTF-8-muotoista.".into())
}

/// Scene `scene`'s tracks and events as JSON (`TimelineSummary`); null and
/// `*error` on failure. Free with `koeta_string_free`.
///
/// # Safety
/// `document` must be null or live; `error` null or writable.
#[no_mangle]
pub unsafe extern "C" fn koeta_timeline_json(
    document: *const Document,
    scene: u32,
    error: *mut *mut c_char,
) -> *mut c_char {
    guarded(error, ptr::null_mut(), || {
        // SAFETY: forwarded caller guarantee.
        let summary = unsafe { super::document(document) }?.timeline(scene as usize)?;
        owned(serde_json::to_string(&summary).map_err(|e| e.to_string())?)
    })
}

/// Start and length of event `event` shifted by `delta` seconds (its end edge
/// with `resize`), snapped to frames, into `*at` and `*duration`.
///
/// # Safety
/// `document` must be null or live; `at` and `duration` writable; `error` null
/// or writable.
#[no_mangle]
pub unsafe extern "C" fn koeta_event_snap(
    document: *const Document,
    scene: u32,
    event: u32,
    delta: f64,
    resize: bool,
    at: *mut f64,
    duration: *mut f64,
    error: *mut *mut c_char,
) -> bool {
    guarded(error, false, || {
        if at.is_null() || duration.is_null() {
            return Err("Tulokselle ei ole paikkaa.".into());
        }
        // SAFETY: forwarded caller guarantee.
        let document = unsafe { super::document(document) }?;
        let (start, length) = document.snap_event(scene as usize, event as usize, delta, resize)?;
        // SAFETY: non-null and writable per the caller contract.
        unsafe {
            *at = start;
            *duration = length;
        }
        Ok(true)
    })
}

/// New source with the event shifted as `koeta_event_snap` shows.
///
/// # Safety
/// As `koeta_timeline_json`.
#[no_mangle]
pub unsafe extern "C" fn koeta_event_move(
    document: *const Document,
    scene: u32,
    event: u32,
    delta: f64,
    resize: bool,
    error: *mut *mut c_char,
) -> *mut c_char {
    guarded(error, ptr::null_mut(), || {
        // SAFETY: forwarded caller guarantee.
        let document = unsafe { super::document(document) }?;
        owned(document.move_event(scene as usize, event as usize, delta, resize)?)
    })
}

/// New source with an event of `kind` (move, gesture, emotion, face or speak)
/// for `actor` at the frame nearest `at` seconds.
///
/// # Safety
/// As `koeta_timeline_json`; `actor` and `kind` NUL-terminated.
#[no_mangle]
pub unsafe extern "C" fn koeta_event_insert(
    document: *const Document,
    scene: u32,
    actor: *const c_char,
    at: f64,
    kind: *const c_char,
    error: *mut *mut c_char,
) -> *mut c_char {
    guarded(error, ptr::null_mut(), || {
        // SAFETY: forwarded caller guarantees.
        let (document, actor, kind) =
            unsafe { (super::document(document)?, c_text(actor)?, c_text(kind)?) };
        owned(document.insert_event(scene as usize, actor, at, self::kind(kind)?)?)
    })
}

/// New source with a copy of the event at the frame nearest `at` seconds.
///
/// # Safety
/// As `koeta_timeline_json`.
#[no_mangle]
pub unsafe extern "C" fn koeta_event_duplicate(
    document: *const Document,
    scene: u32,
    event: u32,
    at: f64,
    error: *mut *mut c_char,
) -> *mut c_char {
    guarded(error, ptr::null_mut(), || {
        // SAFETY: forwarded caller guarantee.
        let document = unsafe { super::document(document) }?;
        owned(document.duplicate_event(scene as usize, event as usize, at)?)
    })
}

/// New source without the event.
///
/// # Safety
/// As `koeta_timeline_json`.
#[no_mangle]
pub unsafe extern "C" fn koeta_event_remove(
    document: *const Document,
    scene: u32,
    event: u32,
    error: *mut *mut c_char,
) -> *mut c_char {
    guarded(error, ptr::null_mut(), || {
        // SAFETY: forwarded caller guarantee.
        let document = unsafe { super::document(document) }?;
        owned(document.remove_event(scene as usize, event as usize)?)
    })
}

/// New source with the event's value (UTF-8, `len` bytes) changed.
///
/// # Safety
/// As `koeta_timeline_json`; `value` must point to `len` readable bytes.
#[no_mangle]
pub unsafe extern "C" fn koeta_event_set_value(
    document: *const Document,
    scene: u32,
    event: u32,
    value: *const u8,
    len: usize,
    error: *mut *mut c_char,
) -> *mut c_char {
    guarded(error, ptr::null_mut(), || {
        // SAFETY: forwarded caller guarantees.
        let (document, value) = unsafe { (super::document(document)?, super::text(value, len)?) };
        owned(document.set_event_value(scene as usize, event as usize, value)?)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = "project:\n  fps: 24\ncharacters:\n  EMMA: { asset: characters/emma, default_pose: idle }\nscenes:\n  - id: one\n    duration: 4\n    events:\n      - { type: gesture, at: 0.5, actor: EMMA, value: wave, duration: 1 }\n      - { type: emotion, at: 1, actor: EMMA, value: happy }\n";

    fn open(source: &str) -> Document {
        Document::open(source.as_bytes()).unwrap()
    }

    #[test]
    fn summary_lists_tracks_events_and_choices() {
        let summary = open(YAML).timeline(0).unwrap();
        assert_eq!((summary.duration, summary.fps), (4.0, 24));
        assert_eq!(summary.actors, ["EMMA"]);
        assert_eq!(summary.events.len(), 2);
        assert_eq!(summary.events[0].kind, "gesture");
        assert_eq!(summary.events[0].value, "wave");
        assert_eq!(summary.events[0].duration, Some(1.0));
        assert_eq!(summary.events[1].kind, "emotion");
        assert_eq!(summary.events[1].duration, None);
        assert!(summary.choices.gesture.contains(&"wave"));
        assert!(summary.choices.emotion.contains(&"happy"));
        let json = serde_json::to_string(&summary).unwrap();
        assert!(json.contains("\"kind\":\"gesture\""));
        assert!(open(YAML).timeline(3).is_err());
    }

    #[test]
    fn moving_and_resizing_snap_like_the_mac_timeline() {
        let document = open(YAML);
        assert_eq!(document.snap_event(0, 0, 0.26, false).unwrap(), (0.75, 1.0));
        let moved = open(&document.move_event(0, 0, 0.26, false).unwrap());
        assert_eq!(moved.timeline(0).unwrap().events[0].at, 0.75);
        let longer = open(&document.move_event(0, 0, 0.4, true).unwrap());
        let event = &longer.timeline(0).unwrap().events[0];
        assert_eq!((event.at, event.duration), (0.5, Some(46.0 / 24.0 - 0.5)));
        assert!(document.move_event(0, 1, 0.5, true).is_err(), "no length");
        assert!(document.move_event(0, 9, 0.5, false).is_err());
        assert!(document.snap_event(0, 0, f64::NAN, false).is_err());
    }

    #[test]
    fn insert_duplicate_remove_and_change_value() {
        let document = open(YAML);
        let added = open(
            &document
                .insert_event(0, "EMMA", 2.01, kind("face").unwrap())
                .unwrap(),
        );
        let events = added.timeline(0).unwrap().events;
        assert_eq!(events.len(), 3);
        assert_eq!((events[2].kind, events[2].at), ("face", 2.0));
        let speech = document.insert_event(0, "EMMA", 1.0, kind("speak").unwrap());
        assert!(
            speech.unwrap_err().contains("voice or imported audio"),
            "a line needs a voice or audio; none is made up"
        );
        assert!(document
            .insert_event(0, "NOBODY", 1.0, Kind::Gesture)
            .is_err());
        assert!(kind("hold").is_err());

        let copied = open(&document.duplicate_event(0, 1, 3.0).unwrap());
        assert_eq!(copied.timeline(0).unwrap().events[2].value, "happy");

        let removed = open(&document.remove_event(0, 0).unwrap());
        assert_eq!(removed.timeline(0).unwrap().events.len(), 1);

        let nod = open(&document.set_event_value(0, 0, "nod").unwrap());
        assert_eq!(nod.timeline(0).unwrap().events[0].value, "nod");
        assert!(document.set_event_value(0, 0, "no-such-gesture").is_err());
        assert!(document.set_event_value(0, 1, "furious").is_err());
        assert_eq!(
            document.source(),
            YAML,
            "edits never change the open document"
        );
    }

    #[test]
    fn c_timeline_interface_round_trips() {
        let document = Box::into_raw(Box::new(open(YAML)));
        let take = |pointer: *mut c_char| {
            assert!(!pointer.is_null());
            // SAFETY: a string from this library, freed once.
            let text = unsafe { CStr::from_ptr(pointer) }
                .to_str()
                .unwrap()
                .to_owned();
            unsafe { crate::koeta_string_free(pointer) };
            text
        };
        let mut error = ptr::null_mut();
        // SAFETY: live document and writable out-pointers throughout.
        unsafe {
            let json = take(koeta_timeline_json(document, 0, &mut error));
            let value: serde_json::Value = serde_json::from_str(&json).unwrap();
            assert_eq!(value["events"][0]["kind"], "gesture");
            let (mut at, mut length) = (0.0, 0.0);
            assert!(koeta_event_snap(
                document,
                0,
                0,
                0.26,
                false,
                &mut at,
                &mut length,
                &mut error
            ));
            assert_eq!((at, length), (0.75, 1.0));
            assert!(
                take(koeta_event_move(document, 0, 0, 0.26, false, &mut error)).contains("0.75")
            );
            let actor = CString::new("EMMA").unwrap();
            let kind = CString::new("face").unwrap();
            assert!(take(koeta_event_insert(
                document,
                0,
                actor.as_ptr(),
                2.0,
                kind.as_ptr(),
                &mut error
            ))
            .contains("face"));
            assert!(!take(koeta_event_remove(document, 0, 1, &mut error)).contains("happy"));
            assert!(
                take(koeta_event_duplicate(document, 0, 1, 3.0, &mut error))
                    .matches("happy")
                    .count()
                    == 2
            );
            let nod = "nod";
            assert!(take(koeta_event_set_value(
                document,
                0,
                0,
                nod.as_ptr(),
                nod.len(),
                &mut error
            ))
            .contains("nod"));
            assert!(error.is_null());
            assert!(koeta_event_remove(document, 0, 9, &mut error).is_null());
            assert!(!error.is_null());
            crate::koeta_string_free(error);
            crate::koeta_document_free(document);
        }
    }
}
