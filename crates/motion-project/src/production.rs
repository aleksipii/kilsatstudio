//! Production tracking: the status and note of each shot.
//!
//! Shots come from `motion_script::shot_list` and are keyed by their id
//! (`<scene id>/<number>`). A record also keeps the shot's start time when it
//! was last updated, so a record whose shot has moved (a cut added or removed
//! earlier in the scene renumbers the shots) can be shown as needing a check
//! instead of silently attaching to another shot.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MAX_SHOT_RECORDS: usize = 2048;
pub const MAX_SHOT_NOTE_CHARS: usize = 2000;
/// A record's start may differ from its shot's by up to this before the
/// record counts as moved (a little more than one frame at 24 fps).
pub const SHOT_START_TOLERANCE: f64 = 0.05;

/// Where a shot is in production, in pipeline order; `Retake` sends it back.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShotStatus {
    Storyboard,
    Animatic,
    Layout,
    Blocking,
    Spline,
    Polish,
    Done,
    Retake,
}
impl ShotStatus {
    pub const ALL: [ShotStatus; 8] = [
        ShotStatus::Storyboard,
        ShotStatus::Animatic,
        ShotStatus::Layout,
        ShotStatus::Blocking,
        ShotStatus::Spline,
        ShotStatus::Polish,
        ShotStatus::Done,
        ShotStatus::Retake,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Storyboard => "storyboard",
            Self::Animatic => "animatic",
            Self::Layout => "layout",
            Self::Blocking => "blocking",
            Self::Spline => "spline",
            Self::Polish => "polish",
            Self::Done => "done",
            Self::Retake => "retake",
        }
    }
    /// Finnish label for the desktop and the storyboard sheet.
    pub fn label(self) -> &'static str {
        match self {
            Self::Storyboard => "Kuvakäsikirjoitus",
            Self::Animatic => "Animatiikki",
            Self::Layout => "Layout",
            Self::Blocking => "Blokkaus",
            Self::Spline => "Välivaiheet",
            Self::Polish => "Viimeistely",
            Self::Done => "Valmis",
            Self::Retake => "Korjattava",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShotRecord {
    pub status: ShotStatus,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// The shot's scene-local start (s) when the record was last updated.
    pub start: f64,
}

/// Shot records keyed by shot id. Empty documents are never written.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProductionDocument {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub shots: BTreeMap<String, ShotRecord>,
}

/// A shot together with its record, as the desktop and the storyboard show it.
#[derive(Debug, Clone, PartialEq)]
pub struct TrackedShot<'a> {
    pub shot: &'a motion_script::ShotInfo,
    pub record: Option<&'a ShotRecord>,
    /// The record was written for a shot that started elsewhere.
    pub moved: bool,
}

fn valid_id(id: &str) -> bool {
    let Some((scene, number)) = id.rsplit_once('/') else {
        return false;
    };
    !scene.is_empty()
        && id.len() <= 160
        && !id.chars().any(char::is_control)
        && number.len() == 3
        && number.bytes().all(|b| b.is_ascii_digit())
}

impl ProductionDocument {
    pub fn is_empty(&self) -> bool {
        self.shots.is_empty()
    }
    pub fn validate(&self) -> Result<(), crate::Error> {
        let fail = |m: String| Err(crate::Error::Validation(format!("production: {m}")));
        if self.shots.len() > MAX_SHOT_RECORDS {
            return fail(format!("more than {MAX_SHOT_RECORDS} shot records"));
        }
        for (id, record) in &self.shots {
            if !valid_id(id) {
                return fail(format!(
                    "shot id {id:?} must be <scene id>/<three digits>, as in kahvila/020"
                ));
            }
            if record.note.chars().count() > MAX_SHOT_NOTE_CHARS {
                return fail(format!(
                    "shot {id}: the note is longer than {MAX_SHOT_NOTE_CHARS} characters"
                ));
            }
            if !record.start.is_finite() || record.start < 0.0 {
                return fail(format!("shot {id}: start must be a finite time ≥ 0"));
            }
        }
        Ok(())
    }
    /// Set a shot's status and note; the record remembers where the shot starts.
    pub fn set(
        &mut self,
        shot: &motion_script::ShotInfo,
        status: ShotStatus,
        note: &str,
    ) -> Result<(), crate::Error> {
        let mut next = self.clone();
        next.shots.insert(
            shot.id.clone(),
            ShotRecord {
                status,
                note: note.to_string(),
                start: shot.start,
            },
        );
        next.validate()?;
        *self = next;
        Ok(())
    }
    /// Every shot with its record, in shot order.
    pub fn track<'a>(&'a self, shots: &'a [motion_script::ShotInfo]) -> Vec<TrackedShot<'a>> {
        shots
            .iter()
            .map(|shot| {
                let record = self.shots.get(&shot.id);
                TrackedShot {
                    shot,
                    record,
                    moved: record
                        .is_some_and(|r| (r.start - shot.start).abs() > SHOT_START_TOLERANCE),
                }
            })
            .collect()
    }
    /// Records whose shot no longer exists.
    pub fn orphans<'a>(&'a self, shots: &[motion_script::ShotInfo]) -> Vec<&'a str> {
        self.shots
            .keys()
            .filter(|id| !shots.iter().any(|s| &s.id == *id))
            .map(String::as_str)
            .collect()
    }
    /// How many shots are at each status; shots without a record count as storyboard.
    pub fn summary(&self, shots: &[motion_script::ShotInfo]) -> BTreeMap<ShotStatus, usize> {
        let mut counts = BTreeMap::new();
        for tracked in self.track(shots) {
            let status = tracked.record.map_or(ShotStatus::Storyboard, |r| r.status);
            *counts.entry(status).or_insert(0) += 1;
        }
        counts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shots() -> Vec<motion_script::ShotInfo> {
        let script = motion_script::parse(
            "characters: {}\nscenes: [{id: kahvila, duration: 6, events: [], composition: {camera: [{at: 0, x: 0, y: 0, zoom: 1}, {at: 2.999, x: 0, y: 0, zoom: 1}, {at: 3, x: 0, y: 0, zoom: 2}]}}]\n",
        )
        .unwrap();
        motion_script::shot_list(&script).unwrap()
    }

    #[test]
    fn records_round_trip_and_empty_documents_are_not_written() {
        let shots = shots();
        let mut doc = ProductionDocument::default();
        assert_eq!(serde_json::to_string(&doc).unwrap(), "{}");
        doc.set(&shots[1], ShotStatus::Blocking, "Kädet liian aikaisin")
            .unwrap();
        let json = serde_json::to_string(&doc).unwrap();
        assert_eq!(
            json,
            r#"{"shots":{"kahvila/020":{"status":"blocking","note":"Kädet liian aikaisin","start":3.0}}}"#
        );
        assert_eq!(
            serde_json::from_str::<ProductionDocument>(&json).unwrap(),
            doc
        );
        let summary = doc.summary(&shots);
        assert_eq!(summary[&ShotStatus::Blocking], 1);
        assert_eq!(summary[&ShotStatus::Storyboard], 1);
    }

    #[test]
    fn moved_and_orphaned_records_are_reported() {
        let shots = shots();
        let mut doc = ProductionDocument::default();
        doc.set(&shots[1], ShotStatus::Done, "").unwrap();
        doc.shots.get_mut("kahvila/020").unwrap().start = 4.0;
        doc.shots.insert(
            "kahvila/090".into(),
            ShotRecord {
                status: ShotStatus::Layout,
                note: String::new(),
                start: 9.0,
            },
        );
        let tracked = doc.track(&shots);
        assert!(!tracked[0].moved && tracked[0].record.is_none());
        assert!(tracked[1].moved);
        assert_eq!(doc.orphans(&shots), ["kahvila/090"]);
    }

    #[test]
    fn bad_records_are_rejected_without_changing_the_document() {
        let shots = shots();
        let mut doc = ProductionDocument::default();
        let long = "x".repeat(MAX_SHOT_NOTE_CHARS + 1);
        assert!(doc.set(&shots[0], ShotStatus::Layout, &long).is_err());
        assert!(doc.is_empty());
        for id in ["kahvila", "kahvila/20", "/010", "kahvila/0a0"] {
            let mut bad = ProductionDocument::default();
            bad.shots.insert(
                id.into(),
                ShotRecord {
                    status: ShotStatus::Layout,
                    note: String::new(),
                    start: 0.0,
                },
            );
            assert!(bad.validate().is_err(), "{id}");
        }
        assert!(serde_json::from_str::<ProductionDocument>(
            r#"{"shots":{"a/010":{"status":"wip","start":0}}}"#
        )
        .is_err());
    }
}
