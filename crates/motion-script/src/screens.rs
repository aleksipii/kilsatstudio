//! Screen cues: what a rig's display slot (`motion_rig::ScreenSlotDefinition`)
//! shows over scene time.
//!
//! A cue starts at scene time `at` and lasts until the next cue for the same
//! actor and slot. Before the first cue the slot shows the rig's own drawing.
//! The source frame is a pure function of scene time, so preview, seeking and
//! export pick the same frame on the shared 24 kHz scene clock.
use crate::{Error, Script};
use motion_rig::{ScreenContent, TextureFit};
use serde::{Deserialize, Serialize};

/// Maximum screen cues per scene.
pub const MAX_SCREEN_CUES: usize = 64;
/// Maximum frames in an image sequence source.
pub const MAX_SCREEN_SEQUENCE_FRAMES: usize = 10_000;
/// Maximum tap markers per cue.
pub const MAX_SCREEN_TAPS: usize = 64;
/// Highest accepted source frame rate.
pub const MAX_SCREEN_FPS: f64 = 240.0;
/// Longest accepted video source, in seconds.
pub const MAX_SCREEN_VIDEO_SECONDS: f64 = 3600.0;
/// Fastest accepted playback speed.
pub const MAX_SCREEN_SPEED: f64 = 16.0;
/// Upper bound on tap times returned for one cue (bounds looping sources).
const MAX_TAP_TIMES: usize = 1024;
/// Video container extensions accepted for screen sources.
pub const SCREEN_VIDEO_EXTENSIONS: [&str; 3] = ["mp4", "mov", "m4v"];

/// Content shown in a screen slot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ScreenSource {
    /// A dark, switched-off screen.
    Off,
    /// One project image asset.
    Image { asset: String },
    /// Project image assets played at `fps`.
    Sequence { frames: Vec<String>, fps: f64 },
    /// A project-relative MP4/MOV recording. Frames are decoded off the UI
    /// thread into a frame cache and addressed by `video_frame_asset`; `fps`
    /// and `duration` describe the decoded frames (seconds).
    Video {
        asset: String,
        fps: f64,
        duration: f64,
    },
}

/// One screen change on the scene clock.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScreenCue {
    pub actor: String,
    /// Slot id on the actor's rig, for example `phone`.
    pub slot: String,
    /// Scene seconds when this content appears.
    pub at: f64,
    pub source: ScreenSource,
    #[serde(default = "default_fit")]
    pub fit: TextureFit,
    /// Source seconds skipped at the start.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub trim_start: f64,
    /// Source seconds where playback ends (holds or loops); omitted: source end.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trim_end: Option<f64>,
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub speed: f64,
    #[serde(default, rename = "loop", skip_serializing_if = "std::ops::Not::not")]
    pub looping: bool,
    /// Source-time tap markers ("tap at 3.2 s"); see `ScreenCue::tap_times`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub taps: Vec<f64>,
}
fn default_fit() -> TextureFit {
    TextureFit::Cover
}
fn one() -> f64 {
    1.0
}
fn is_one(v: &f64) -> bool {
    *v == 1.0
}
fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

/// Asset id of decoded frame `frame` of a screen video. Frame caches and
/// image resource maps key decoded frames by this id.
pub fn video_frame_asset(asset: &str, frame: u64) -> String {
    format!("{asset}#frame={frame}")
}

impl ScreenSource {
    /// Playable source length in seconds; `None` for stills and `off`.
    pub fn length(&self) -> Option<f64> {
        match self {
            Self::Off | Self::Image { .. } => None,
            Self::Sequence { frames, fps } => Some(frames.len() as f64 / fps),
            Self::Video { fps, duration, .. } => {
                Some(Self::video_frames(*fps, *duration) as f64 / fps)
            }
        }
    }
    /// Number of decoded frames for a video of `duration` seconds at `fps`.
    pub fn video_frames(fps: f64, duration: f64) -> u64 {
        ((duration * fps).round() as u64).max(1)
    }
}

impl ScreenCue {
    fn end(&self) -> Option<f64> {
        self.source.length().map(|l| self.trim_end.unwrap_or(l))
    }
    /// Source seconds shown at scene time `time`; `None` before `at` or for
    /// sources without time. A finished non-looping source holds its last frame.
    pub fn source_time(&self, time: f64) -> Option<f64> {
        let end = self.end()?;
        if time.is_nan() || time < self.at {
            return None;
        }
        let span = end - self.trim_start;
        let elapsed = (time - self.at) * self.speed;
        Some(if self.looping {
            self.trim_start + elapsed.rem_euclid(span)
        } else {
            self.trim_start + elapsed.min(span)
        })
    }
    fn frame(&self, time: f64, fps: f64, count: u64) -> u64 {
        let t = self.source_time(time).unwrap_or(self.trim_start);
        // The epsilon keeps exact frame boundaries from rounding down.
        ((t * fps + 1e-9).floor().max(0.0) as u64).min(count.saturating_sub(1))
    }
    /// The decoded video frame `(asset, frame)` this cue needs at `time`, if
    /// its source is a video. Frame loaders use this to fetch only what a
    /// frame shows.
    pub fn video_frame_at(&self, time: f64) -> Option<(&str, u64)> {
        match &self.source {
            ScreenSource::Video {
                asset,
                fps,
                duration,
            } => Some((
                asset.as_str(),
                self.frame(time, *fps, ScreenSource::video_frames(*fps, *duration)),
            )),
            _ => None,
        }
    }
    /// What the slot shows at scene time `time` (at or after `at`).
    pub fn content_at(&self, time: f64) -> ScreenContent {
        let frame = |fps: f64, count: u64| self.frame(time, fps, count);
        let image = |asset: String| ScreenContent::Image {
            asset,
            fit: self.fit,
        };
        match &self.source {
            ScreenSource::Off => ScreenContent::Off,
            ScreenSource::Image { asset } => image(asset.clone()),
            ScreenSource::Sequence { frames, fps } => {
                let index = frame(*fps, frames.len() as u64) as usize;
                image(frames[index].clone())
            }
            ScreenSource::Video {
                asset,
                fps,
                duration,
            } => {
                let count = ScreenSource::video_frames(*fps, *duration);
                image(video_frame_asset(asset, frame(*fps, count)))
            }
        }
    }
    /// Scene times at which the source shows a tap marker, up to `until`
    /// (looping sources repeat their taps). Automatic direction can time a
    /// finger press to these.
    pub fn tap_times(&self, until: f64) -> Vec<f64> {
        let Some(end) = self.end() else {
            return Vec::new();
        };
        let span = end - self.trim_start;
        let mut times = Vec::new();
        for pass in 0u64.. {
            let base = self.at + pass as f64 * span / self.speed;
            if base > until || times.len() >= MAX_TAP_TIMES || (pass > 0 && !self.looping) {
                break;
            }
            for tap in &self.taps {
                let at = base + (tap - self.trim_start) / self.speed;
                if *tap >= self.trim_start && *tap < end && at <= until {
                    times.push(at);
                }
            }
        }
        times.truncate(MAX_TAP_TIMES);
        times
    }

    fn validate(&self, script: &Script, scene: &str, duration: f64) -> Result<(), Error> {
        let fail = |m: String| {
            Error::Validation(format!(
                "{scene}: screen cue {}.{} at {}: {m}",
                self.actor, self.slot, self.at
            ))
        };
        let has_slot = script
            .characters
            .get(&self.actor)
            // `holds:` props were validated with the characters; their slots count.
            .and_then(|c| c.resolved_rig().ok().flatten())
            .is_some_and(|rig| rig.screens.iter().any(|s| s.id == self.slot));
        if !has_slot {
            return Err(fail(format!(
                "actor {} has no screen slot {}; hold a phone or tablet (holds: {{item: puhelin, hand: right}}) or add a slot to its rig",
                self.actor, self.slot
            )));
        }
        if !self.at.is_finite() || self.at < 0.0 || self.at >= duration {
            return Err(fail("at must be finite and within the scene".into()));
        }
        let image_id = |id: &str| {
            !id.trim().is_empty() && id.len() <= 256 && !id.chars().any(char::is_control)
        };
        let finite_rate = |fps: f64| fps.is_finite() && fps > 0.0 && fps <= MAX_SCREEN_FPS;
        match &self.source {
            ScreenSource::Off => {}
            ScreenSource::Image { asset } => {
                if !image_id(asset) {
                    return Err(fail("image asset id is empty or invalid".into()));
                }
            }
            ScreenSource::Sequence { frames, fps } => {
                if frames.is_empty()
                    || frames.len() > MAX_SCREEN_SEQUENCE_FRAMES
                    || !frames.iter().all(|f| image_id(f))
                {
                    return Err(fail(format!(
                        "sequence needs 1..={MAX_SCREEN_SEQUENCE_FRAMES} valid image asset ids"
                    )));
                }
                if !finite_rate(*fps) {
                    return Err(fail(format!("fps must be within (0, {MAX_SCREEN_FPS}]")));
                }
            }
            ScreenSource::Video {
                asset,
                fps,
                duration: length,
            } => {
                let path = std::path::Path::new(asset);
                if asset.trim().is_empty()
                    || asset.len() > 512
                    || asset.contains(['\\', '\0', '#'])
                    || !path
                        .components()
                        .all(|c| matches!(c, std::path::Component::Normal(_)))
                {
                    return Err(fail(
                        "invalid video asset: use a project-relative path without '..' or '#'"
                            .into(),
                    ));
                }
                let extension = path
                    .extension()
                    .and_then(|e| e.to_str())
                    .map(str::to_ascii_lowercase);
                if !extension.is_some_and(|e| SCREEN_VIDEO_EXTENSIONS.contains(&e.as_str())) {
                    return Err(Error::Unsupported(format!(
                        "{scene}: screen cue {}.{}: only .mp4, .mov and .m4v videos are supported",
                        self.actor, self.slot
                    )));
                }
                if !finite_rate(*fps)
                    || !length.is_finite()
                    || *length <= 0.0
                    || *length > MAX_SCREEN_VIDEO_SECONDS
                {
                    return Err(fail(format!(
                        "video needs fps within (0, {MAX_SCREEN_FPS}] and duration within (0, {MAX_SCREEN_VIDEO_SECONDS}] seconds"
                    )));
                }
            }
        }
        if !self.speed.is_finite() || self.speed <= 0.0 || self.speed > MAX_SCREEN_SPEED {
            return Err(fail(format!(
                "speed must be within (0, {MAX_SCREEN_SPEED}]"
            )));
        }
        let Some(length) = self.source.length() else {
            if self.trim_start != 0.0
                || self.trim_end.is_some()
                || self.speed != 1.0
                || self.looping
                || !self.taps.is_empty()
            {
                return Err(fail(
                    "trim, speed, loop and taps apply only to sequence and video sources".into(),
                ));
            }
            return Ok(());
        };
        if !self.trim_start.is_finite() || self.trim_start < 0.0 || self.trim_start >= length {
            return Err(fail(format!(
                "trim_start must be within [0, {length}) source seconds"
            )));
        }
        let end = self.trim_end.unwrap_or(length);
        if !end.is_finite() || end <= self.trim_start || end > length {
            return Err(fail(format!(
                "trim_end must be after trim_start and at most {length} source seconds"
            )));
        }
        if self.taps.len() > MAX_SCREEN_TAPS
            || !self.taps.windows(2).all(|w| w[0] < w[1])
            || !self
                .taps
                .iter()
                .all(|t| t.is_finite() && *t >= self.trim_start && *t < end)
        {
            return Err(fail(format!(
                "taps need at most {MAX_SCREEN_TAPS} increasing source times within the trimmed range"
            )));
        }
        Ok(())
    }
}

impl crate::Composition {
    /// What `actor`'s slot `slot` shows at scene time `time`: the latest cue at
    /// or before `time`, or the rig's own drawing before any cue.
    pub fn screen_content(&self, actor: &str, slot: &str, time: f64) -> ScreenContent {
        self.screens
            .iter()
            .filter(|c| c.actor == actor && c.slot == slot && c.at <= time)
            .max_by(|a, b| a.at.total_cmp(&b.at))
            .map_or(ScreenContent::Drawn, |cue| cue.content_at(time))
    }
}

impl crate::Composition {
    /// Decoded video frames `(asset, frame)` that the scene shows at `time`.
    pub fn video_frames_at(&self, time: f64) -> Vec<(&str, u64)> {
        let mut active: Vec<&ScreenCue> = Vec::new();
        for cue in self.screens.iter().filter(|c| c.at <= time) {
            match active
                .iter_mut()
                .find(|a| a.actor == cue.actor && a.slot == cue.slot)
            {
                Some(current) if current.at < cue.at => *current = cue,
                Some(_) => {}
                None => active.push(cue),
            }
        }
        active
            .iter()
            .filter_map(|c| c.video_frame_at(time))
            .collect()
    }
}

impl Script {
    /// Project image assets that screen cues show directly (stills and
    /// sequence frames). Decoded video frames are not project assets.
    pub fn screen_image_assets(&self) -> impl Iterator<Item = &str> {
        self.scenes
            .iter()
            .flat_map(|s| &s.composition.screens)
            .flat_map(|cue| -> Vec<&str> {
                match &cue.source {
                    ScreenSource::Image { asset } => vec![asset.as_str()],
                    ScreenSource::Sequence { frames, .. } => {
                        frames.iter().map(String::as_str).collect()
                    }
                    _ => Vec::new(),
                }
            })
    }

    pub(crate) fn validate_screens(&self, scene: &crate::Scene) -> Result<(), Error> {
        let cues = &scene.composition.screens;
        if cues.len() > MAX_SCREEN_CUES {
            return Err(Error::Validation(format!(
                "{}: at most {MAX_SCREEN_CUES} screen cues per scene",
                scene.id
            )));
        }
        for (index, cue) in cues.iter().enumerate() {
            cue.validate(self, &scene.id, scene.duration)?;
            if cues[..index]
                .iter()
                .any(|c| c.actor == cue.actor && c.slot == cue.slot && c.at == cue.at)
            {
                return Err(Error::Validation(format!(
                    "{}: two screen cues for {}.{} start at {}",
                    scene.id, cue.actor, cue.slot, cue.at
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT: &str = r#"
characters:
  liisa:
    asset: custom
    rig:
      version: 1
      id: liisa
      joints: [{id: hand_r, parent: null}]
      parts: [{id: glass, joint: hand_r, points: [[0,0],[9,0],[9,19.5],[0,19.5]], color: [9,9,9,255]}]
      screens: [{id: phone, part: glass, origin: [0,0], x_axis: [9,0], y_axis: [0,19.5]}]
scenes:
  - id: demo
    duration: 10
    events: []
    composition:
      screens:
        - actor: liisa
          slot: phone
          at: 2.0
          source: {type: video, asset: assets/screens/oma-sovellus-demo.mov, fps: 30, duration: 4}
          trim_start: 1
          trim_end: 3
          speed: 2
          taps: [2.5]
"#;
    fn script() -> Script {
        crate::parse(SCRIPT).unwrap()
    }
    fn with_cue(edit: impl FnOnce(&mut ScreenCue)) -> Result<(), Error> {
        let mut s = script();
        edit(&mut s.scenes[0].composition.screens[0]);
        s.validate()
    }

    #[test]
    fn yaml_cue_parses_with_cover_default_and_round_trips() {
        let s = script();
        s.validate().unwrap();
        let cue = &s.scenes[0].composition.screens[0];
        assert_eq!(cue.fit, TextureFit::Cover);
        assert!(!cue.looping);
        let yaml = serde_yaml::to_string(&s.scenes[0].composition).unwrap();
        let back: crate::Composition = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.screens, s.scenes[0].composition.screens);
    }

    #[test]
    fn trimmed_fast_playback_holds_the_last_frame() {
        let s = script();
        let cue = &s.scenes[0].composition.screens[0];
        assert_eq!(cue.source_time(1.9), None);
        assert_eq!(cue.source_time(2.0), Some(1.0));
        assert_eq!(cue.source_time(2.5), Some(2.0));
        // 2 s of source at double speed lasts 1 s, then holds trim_end.
        assert_eq!(cue.source_time(9.0), Some(3.0));
        assert_eq!(cue.video_frame_at(2.5).unwrap().1, 60);
        // trim_end 3.0 s is frame 90 of 120: still a valid frame.
        assert_eq!(cue.video_frame_at(9.0).unwrap().1, 90);
        assert_eq!(cue.tap_times(10.0), vec![2.75]);
    }

    #[test]
    fn sequences_pick_frames_and_loop() {
        let cue = ScreenCue {
            actor: "a".into(),
            slot: "phone".into(),
            at: 0.0,
            source: ScreenSource::Sequence {
                frames: vec!["f0".into(), "f1".into(), "f2".into()],
                fps: 2.0,
            },
            fit: TextureFit::Contain,
            trim_start: 0.0,
            trim_end: None,
            speed: 1.0,
            looping: true,
            taps: vec![],
        };
        let asset = |t: f64| match cue.content_at(t) {
            ScreenContent::Image { asset, .. } => asset,
            other => panic!("{other:?}"),
        };
        assert_eq!(
            [asset(0.0), asset(0.5), asset(1.49), asset(1.5)],
            ["f0", "f1", "f2", "f0"]
        );
    }

    #[test]
    fn invalid_cues_name_the_problem() {
        let message = |r: Result<(), Error>| r.unwrap_err().to_string();
        assert!(message(with_cue(|c| c.slot = "tv".into())).contains("has no screen slot tv"));
        assert!(message(with_cue(|c| c.at = 10.0)).contains("within the scene"));
        assert!(message(with_cue(|c| c.speed = 0.0)).contains("speed"));
        assert!(message(with_cue(|c| c.trim_end = Some(5.0))).contains("trim_end"));
        assert!(message(with_cue(|c| c.taps = vec![0.5])).contains("taps"));
        assert!(message(with_cue(|c| c.taps = vec![2.0, 1.5])).contains("taps"));
        assert!(message(with_cue(|c| {
            c.source = ScreenSource::Video {
                asset: "../outside.mov".into(),
                fps: 30.0,
                duration: 4.0,
            }
        }))
        .contains("project-relative"));
        let unsupported = with_cue(|c| {
            c.source = ScreenSource::Video {
                asset: "clip.avi".into(),
                fps: 30.0,
                duration: 4.0,
            }
        });
        assert!(matches!(unsupported, Err(Error::Unsupported(_))));
        assert!(message(with_cue(|c| {
            c.source = ScreenSource::Image {
                asset: "still".into(),
            }
        }))
        .contains("apply only to sequence and video"));
        let mut s = script();
        let cue = s.scenes[0].composition.screens[0].clone();
        s.scenes[0].composition.screens.push(cue);
        assert!(s
            .validate()
            .unwrap_err()
            .to_string()
            .contains("two screen cues"));
        // Unknown fields are syntax errors, not silently ignored.
        assert!(crate::parse(&SCRIPT.replace("speed: 2", "sped: 2")).is_err());
    }

    #[test]
    fn removing_the_character_removes_its_screen_cues() {
        let mut s = script();
        s.characters
            .insert("other".into(), s.characters["liisa"].clone());
        let source = serde_yaml::to_string(&s).unwrap();
        let edited = crate::parse(&crate::remove_character(&source, "liisa").unwrap()).unwrap();
        assert!(edited.scenes[0].composition.screens.is_empty());
    }
}
