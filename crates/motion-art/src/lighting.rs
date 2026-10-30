//! Deterministic scene lighting and animated-image props.
//!
//! Lights are plain data on the artwork `Document`. They are rendered by the
//! shared SVG compositor with existing blend modes and radial gradients. Every
//! time-dependent value (tracks, flicker, frame selection) is a pure function of
//! scene-local seconds: no clock, RNG or global state.
use crate::{
    coordinate, identifier, invalid, BlendMode, Color, Error, Point, TimeRange, MAX_COORDINATE,
};
use serde::{Deserialize, Serialize};

pub const MAX_LIGHTS: usize = 64;
pub const MAX_LIGHTS_PER_SCENE: usize = 16;
pub const MAX_LIGHT_KEYS: usize = 128;
pub const MAX_TOTAL_LIGHT_KEYS: usize = 2048;
pub const MAX_LIGHT_RADIUS: f64 = 20_000.0;
/// Work units charged per light and rendered frame by export preflight.
pub const LIGHT_FRAME_COST: u64 = 24;
pub const MAX_ANIMATED_FRAMES: usize = 256;
pub const MAX_TOTAL_ANIMATED_FRAMES: usize = 4096;
pub const MIN_FRAME_SECONDS: f64 = 0.001;
pub const MAX_FRAME_SECONDS: f64 = 3600.0;
pub const MAX_SEQUENCE_SECONDS: f64 = 86_400.0;

/// What happens after the last frame of an animated image.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Playback {
    /// Show the last frame forever.
    #[default]
    Hold,
    /// Restart from the first frame.
    Loop,
}

/// One project image asset shown for `duration` scene seconds.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ImageFrame {
    pub asset_id: String,
    pub duration: f64,
}

/// Validate a frame list; used by `Document::validate`.
pub(crate) fn validate_frames(frames: &[ImageFrame]) -> Result<(), Error> {
    if frames.is_empty() || frames.len() > MAX_ANIMATED_FRAMES {
        return invalid("animated image needs 1-256 frames");
    }
    let mut total = 0.0;
    for frame in frames {
        if !identifier(&frame.asset_id)
            || !frame.duration.is_finite()
            || !(MIN_FRAME_SECONDS..=MAX_FRAME_SECONDS).contains(&frame.duration)
        {
            return invalid("animated image frame requires an asset id and duration 0.001-3600 s");
        }
        total += frame.duration;
    }
    if total > MAX_SEQUENCE_SECONDS {
        return invalid("animated image total duration exceeds 86400 s");
    }
    Ok(())
}

/// Frame active at `local` seconds after the prop starts. Frame `i` covers the
/// half-open interval `[start_i, end_i)`, so an exact boundary selects the next
/// frame. Boundaries are cumulative f64 sums in list order.
pub fn select_frame(frames: &[ImageFrame], playback: Playback, local: f64) -> Option<&ImageFrame> {
    let last = frames.last()?;
    if !local.is_finite() {
        return None;
    }
    let total: f64 = frames.iter().map(|f| f.duration).sum();
    let mut t = local.max(0.0);
    if playback == Playback::Loop && total > 0.0 {
        t %= total;
    }
    let mut end = 0.0;
    for frame in frames {
        end += frame.duration;
        if t < end {
            return Some(frame);
        }
    }
    Some(last)
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FlickerMode {
    #[default]
    Sine,
    Noise,
}

/// Deterministic intensity modulation: `1 - amount * w(t)` with `w` in `[0, 1]`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Flicker {
    #[serde(default)]
    pub mode: FlickerMode,
    pub amount: f64,
    pub rate_hz: f64,
    #[serde(default)]
    pub seed: u32,
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}
fn unit_hash(seed: u32, n: u64) -> f64 {
    let h = splitmix((u64::from(seed) << 32) ^ splitmix(n));
    (h >> 11) as f64 / (1u64 << 53) as f64
}

impl Flicker {
    pub fn validate(&self) -> Result<(), Error> {
        if !self.amount.is_finite()
            || !(0.0..=1.0).contains(&self.amount)
            || !self.rate_hz.is_finite()
            || !(0.01..=60.0).contains(&self.rate_hz)
        {
            return invalid("flicker requires amount 0-1 and rate 0.01-60 Hz");
        }
        Ok(())
    }
    /// Multiplier in `[1 - amount, 1]` at scene-local `time`.
    pub fn factor(&self, time: f64) -> f64 {
        let t = if time.is_finite() { time.max(0.0) } else { 0.0 };
        let wave = match self.mode {
            FlickerMode::Sine => {
                let phase = unit_hash(self.seed, 0) * std::f64::consts::TAU;
                0.5 + 0.5 * (std::f64::consts::TAU * self.rate_hz * t + phase).sin()
            }
            FlickerMode::Noise => {
                let u = t * self.rate_hz;
                let cell = u.floor();
                let f = u - cell;
                let s = f * f * (3.0 - 2.0 * f);
                let n = cell as u64;
                let (a, b) = (unit_hash(self.seed, n), unit_hash(self.seed, n + 1));
                a + (b - a) * s
            }
        };
        1.0 - self.amount * wave
    }
}

/// One key of a light's position track (scene space, seconds from scene start).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct LightKey {
    pub time: f64,
    pub x: f64,
    pub y: f64,
}

/// Tie a light to a prop layer showing an image: colour follows the shown
/// frame's average colour and intensity scales with its luminance.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ScreenGlow {
    pub layer_id: String,
    /// Multiplier on the light intensity (0-4).
    pub gain: f64,
    /// 0 keeps the light colour; 1 uses the frame's average colour.
    pub tint: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum LightSource {
    /// Whole-canvas multiply tint; `strength` 0 is invisible, 1 is full colour.
    Ambient { color: Color, strength: f64 },
    /// Darkening toward the canvas edge; `inner` is the clear central fraction.
    Vignette { strength: f64, inner: f64 },
    /// Soft radial light. Scene-space `position`; `softness` 0 is a hard disc,
    /// 1 falls off smoothly from the centre. With `screen_glow`, `position` is an
    /// offset from the prop's rectangle centre.
    Radial {
        position: Point,
        radius: f64,
        #[serde(default = "one")]
        aspect: f64,
        #[serde(default = "one")]
        softness: f64,
        color: Color,
        intensity: f64,
        #[serde(default = "screen")]
        blend: BlendMode,
        /// Actor translation is added to the position (like layer attachment).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attachment: Option<String>,
        /// Optional animated position replacing `position` (linear, endpoints hold).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        track: Vec<LightKey>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        screen_glow: Option<ScreenGlow>,
    },
}
fn one() -> f64 {
    1.0
}
fn screen() -> BlendMode {
    BlendMode::Screen
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct Light {
    pub id: String,
    pub scene_id: String,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<TimeRange>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub flicker: Option<Flicker>,
    /// Draw after background artwork but before actors and foreground props.
    /// Defaults to the historical overlay behavior for existing projects.
    #[serde(default, skip_serializing_if = "is_false")]
    pub behind_actors: bool,
    pub source: LightSource,
}
fn is_false(value: &bool) -> bool {
    !value
}
fn yes() -> bool {
    true
}

impl Light {
    pub fn is_active(&self, time: f64) -> bool {
        self.visible
            && time.is_finite()
            && time >= 0.0
            && self
                .active
                .is_none_or(|range| range.start <= time && time < range.end)
    }
    /// Flicker multiplier (1 without flicker).
    pub fn flicker_factor(&self, time: f64) -> f64 {
        self.flicker.map_or(1.0, |f| f.factor(time))
    }
    pub fn key_count(&self) -> usize {
        match &self.source {
            LightSource::Radial { track, .. } => track.len(),
            _ => 0,
        }
    }
    /// Scene-space position before attachment/screen offsets; track overrides base.
    pub fn sample_position(&self, time: f64) -> Option<Point> {
        let LightSource::Radial {
            position, track, ..
        } = &self.source
        else {
            return None;
        };
        let (Some(first), Some(last)) = (track.first(), track.last()) else {
            return Some(*position);
        };
        if time <= first.time {
            return Some(Point {
                x: first.x,
                y: first.y,
            });
        }
        if time >= last.time {
            return Some(Point {
                x: last.x,
                y: last.y,
            });
        }
        let i = track.partition_point(|k| k.time <= time);
        let (a, b) = (track[i - 1], track[i]);
        let f = (time - a.time) / (b.time - a.time);
        Some(Point {
            x: a.x + (b.x - a.x) * f,
            y: a.y + (b.y - a.y) * f,
        })
    }
    pub fn validate(&self) -> Result<(), Error> {
        if !identifier(&self.id) || !identifier(&self.scene_id) {
            return invalid("light identifiers must be valid");
        }
        if self.active.is_some_and(|r| {
            !r.start.is_finite() || !r.end.is_finite() || r.start < 0.0 || r.start >= r.end
        }) {
            return invalid("light active interval requires finite nonnegative start < end");
        }
        if let Some(f) = &self.flicker {
            f.validate()?;
        }
        let rgb = |c: &Color| c.a == 255;
        match &self.source {
            LightSource::Ambient { color, strength } => {
                if !rgb(color) || !strength.is_finite() || !(0.0..=1.0).contains(strength) {
                    return invalid("ambient light requires opaque colour and strength 0-1");
                }
            }
            LightSource::Vignette { strength, inner } => {
                if !strength.is_finite()
                    || !(0.0..=1.0).contains(strength)
                    || !inner.is_finite()
                    || !(0.0..1.0).contains(inner)
                {
                    return invalid("vignette requires strength 0-1 and inner 0 to below 1");
                }
            }
            LightSource::Radial {
                position,
                radius,
                aspect,
                softness,
                color,
                intensity,
                attachment,
                track,
                screen_glow,
                ..
            } => {
                if !coordinate(position.x)
                    || !coordinate(position.y)
                    || !radius.is_finite()
                    || !(0.5..=MAX_LIGHT_RADIUS).contains(radius)
                    || !aspect.is_finite()
                    || !(0.1..=10.0).contains(aspect)
                    || !softness.is_finite()
                    || !(0.0..=1.0).contains(softness)
                    || !rgb(color)
                    || !intensity.is_finite()
                    || !(0.0..=1.0).contains(intensity)
                    || attachment.as_ref().is_some_and(|id| !identifier(id))
                {
                    return invalid("radial light requires bounded position, radius 0.5-20000, aspect 0.1-10, softness 0-1, opaque colour and intensity 0-1");
                }
                if track.len() > MAX_LIGHT_KEYS {
                    return invalid("light track exceeds 128 keys");
                }
                let mut previous: Option<f64> = None;
                for key in track {
                    if !key.time.is_finite()
                        || key.time < 0.0
                        || previous.is_some_and(|p| key.time <= p)
                        || !coordinate(key.x)
                        || !coordinate(key.y)
                        || key.time > MAX_COORDINATE
                    {
                        return invalid("light track keys require increasing finite nonnegative seconds and bounded positions");
                    }
                    previous = Some(key.time);
                }
                if let Some(glow) = screen_glow {
                    if !identifier(&glow.layer_id)
                        || !glow.gain.is_finite()
                        || !(0.0..=4.0).contains(&glow.gain)
                        || !glow.tint.is_finite()
                        || !(0.0..=1.0).contains(&glow.tint)
                    {
                        return invalid("screen glow requires a layer id, gain 0-4 and tint 0-1");
                    }
                }
            }
        }
        Ok(())
    }
    /// Scene-dependent bounds: interval and track keys within the scene.
    pub fn validate_scene_duration(&self, duration: f64) -> Result<(), Error> {
        if self.active.is_some_and(|r| r.end > duration) {
            return invalid("light active interval exceeds scene duration");
        }
        if let LightSource::Radial { track, .. } = &self.source {
            if track.last().is_some_and(|k| k.time > duration) {
                return invalid("light track key exceeds scene duration");
            }
        }
        Ok(())
    }
}

/// Per-frame preflight work for the lights of one scene.
pub fn lighting_frame_cost(lights: &[Light], scene_id: &str) -> u64 {
    lights.iter().filter(|l| l.scene_id == scene_id).count() as u64 * LIGHT_FRAME_COST
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Content, Document, Layer, Plane, Rect};

    fn frames(spec: &[(&str, f64)]) -> Vec<ImageFrame> {
        spec.iter()
            .map(|(a, d)| ImageFrame {
                asset_id: (*a).into(),
                duration: *d,
            })
            .collect()
    }
    fn black() -> Color {
        Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }
    #[test]
    fn backlight_flag_roundtrips_and_legacy_lights_stay_overlays() {
        let mut light = lamp();
        let legacy = serde_json::to_value(&light).unwrap();
        assert!(legacy.get("behind_actors").is_none());
        let restored: Light = serde_json::from_value(legacy).unwrap();
        assert!(!restored.behind_actors);
        light.behind_actors = true;
        let restored: Light =
            serde_json::from_value(serde_json::to_value(&light).unwrap()).unwrap();
        assert_eq!(restored, light);
    }

    fn lamp() -> Light {
        Light {
            id: "lamp".into(),
            scene_id: "s".into(),
            visible: true,
            active: None,
            flicker: None,
            behind_actors: false,
            source: LightSource::Radial {
                position: Point { x: 1.0, y: 2.0 },
                radius: 50.0,
                aspect: 1.0,
                softness: 1.0,
                color: Color {
                    r: 255,
                    g: 200,
                    b: 100,
                    a: 255,
                },
                intensity: 0.5,
                blend: BlendMode::Screen,
                attachment: None,
                track: Vec::new(),
                screen_glow: None,
            },
        }
    }
    fn prop(id: &str) -> Layer {
        Layer {
            id: id.into(),
            scene_id: "s".into(),
            name: id.into(),
            visible: true,
            plane: Plane::Foreground,
            active: None,
            opacity: 1.0,
            attachment: None,
            clip: None,
            clip_shape: None,
            blend: BlendMode::Normal,
            content: Content::AnimatedImage {
                frames: frames(&[("a", 0.5), ("b", 0.25)]),
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
                playback: Playback::Hold,
            },
            angle: None,
        }
    }
    fn flicker(mode: FlickerMode, amount: f64, rate_hz: f64, seed: u32) -> Flicker {
        Flicker {
            mode,
            amount,
            rate_hz,
            seed,
        }
    }
    fn key(time: f64, x: f64, y: f64) -> LightKey {
        LightKey { time, x, y }
    }
    fn edited(edit: impl Fn(&mut Light)) -> bool {
        let mut l = lamp();
        edit(&mut l);
        l.validate().is_err()
    }
    fn radial_mut(l: &mut Light) -> &mut LightSource {
        &mut l.source
    }

    #[test]
    fn frame_selection_boundaries() {
        let list = frames(&[("a", 0.5), ("b", 0.25), ("c", 1.0)]);
        let at = |p, t| select_frame(&list, p, t).map(|f| f.asset_id.as_str());
        assert_eq!(at(Playback::Hold, -1.0), Some("a"));
        assert_eq!(at(Playback::Hold, 0.0), Some("a"));
        assert_eq!(at(Playback::Hold, 0.4999), Some("a"));
        assert_eq!(at(Playback::Hold, 0.5), Some("b"));
        assert_eq!(at(Playback::Hold, 0.75), Some("c"));
        assert_eq!(at(Playback::Hold, 1000.0), Some("c"));
        assert_eq!(at(Playback::Loop, 1.75), Some("a"));
        assert_eq!(at(Playback::Loop, 2.3), Some("b"));
        assert_eq!(at(Playback::Hold, f64::NAN), None);
        assert!(select_frame(&[], Playback::Hold, 0.0).is_none());
    }

    #[test]
    fn flicker_is_pure_bounded_and_seed_dependent() {
        for mode in [FlickerMode::Sine, FlickerMode::Noise] {
            let f = flicker(mode, 0.6, 5.0, 11);
            for i in 0..200 {
                let t = f64::from(i) * 0.013;
                let v = f.factor(t);
                assert!((0.4..=1.0).contains(&v), "{v}");
                assert_eq!(v.to_bits(), f.factor(t).to_bits());
            }
            let g = Flicker { seed: 12, ..f };
            assert!((0..200).any(|i| {
                let t = f64::from(i) * 0.013;
                f.factor(t) != g.factor(t)
            }));
        }
        assert_eq!(flicker(FlickerMode::Noise, 0.0, 5.0, 0).factor(1.234), 1.0);
        // Cell boundary of the noise mode equals the integer hash exactly.
        let pinned = flicker(FlickerMode::Noise, 1.0, 1.0, 1);
        assert_eq!(
            pinned.factor(0.0).to_bits(),
            (1.0 - unit_hash(1, 0)).to_bits()
        );
    }

    #[test]
    fn track_interpolates_and_holds() {
        let mut l = lamp();
        if let LightSource::Radial { track, .. } = &mut l.source {
            *track = vec![key(1.0, 0.0, 0.0), key(3.0, 100.0, 50.0)];
        }
        assert_eq!(l.sample_position(0.0), Some(Point { x: 0.0, y: 0.0 }));
        assert_eq!(l.sample_position(2.0), Some(Point { x: 50.0, y: 25.0 }));
        assert_eq!(l.sample_position(9.0), Some(Point { x: 100.0, y: 50.0 }));
        assert_eq!(lamp().sample_position(5.0), Some(Point { x: 1.0, y: 2.0 }));
    }

    #[test]
    fn validation_rejects_bad_lights_and_frames() {
        let mut doc = Document {
            lights: vec![lamp()],
            layers: vec![prop("phone")],
            ..Default::default()
        };
        doc.validate().unwrap();
        assert!(edited(
            |l| l.flicker = Some(flicker(FlickerMode::Sine, f64::NAN, 1.0, 0))
        ));
        assert!(edited(
            |l| l.flicker = Some(flicker(FlickerMode::Sine, 0.5, 0.0, 0))
        ));
        assert!(edited(|l| l.active = Some(TimeRange {
            start: 2.0,
            end: 1.0
        })));
        assert!(edited(|l| {
            if let LightSource::Radial { radius, .. } = radial_mut(l) {
                *radius = f64::INFINITY;
            }
        }));
        assert!(edited(|l| {
            if let LightSource::Radial { intensity, .. } = radial_mut(l) {
                *intensity = 1.5;
            }
        }));
        assert!(edited(|l| {
            if let LightSource::Radial { color, .. } = radial_mut(l) {
                color.a = 10;
            }
        }));
        assert!(edited(|l| {
            if let LightSource::Radial { track, .. } = radial_mut(l) {
                *track = vec![key(1.0, 0.0, 0.0), key(1.0, 0.0, 0.0)];
            }
        }));
        assert!(edited(|l| l.source = LightSource::Ambient {
            color: black(),
            strength: 2.0
        }));
        assert!(edited(|l| l.source = LightSource::Vignette {
            strength: 0.5,
            inner: 1.0
        }));
        for list in [
            frames(&[]),
            frames(&[("a", 0.0)]),
            frames(&[("a", f64::NAN)]),
            frames(&[("", 1.0)]),
            frames(&[("a", 4000.0)]),
        ] {
            assert!(validate_frames(&list).is_err(), "{list:?}");
        }
        let many: Vec<ImageFrame> = (0..257)
            .map(|_| ImageFrame {
                asset_id: "a".into(),
                duration: 0.1,
            })
            .collect();
        assert!(validate_frames(&many).is_err());
        let mut dup = doc.clone();
        dup.lights.push(lamp());
        assert!(dup.validate().is_err());
        let mut crowded = Document::default();
        for i in 0..=MAX_LIGHTS_PER_SCENE {
            let mut l = lamp();
            l.id = format!("l{i}");
            crowded.lights.push(l);
        }
        assert!(crowded
            .validate()
            .unwrap_err()
            .to_string()
            .contains("16 lights"));
        let mut glow = lamp();
        if let LightSource::Radial { screen_glow, .. } = &mut glow.source {
            *screen_glow = Some(ScreenGlow {
                layer_id: "phone".into(),
                gain: 1.0,
                tint: 1.0,
            });
        }
        glow.id = "glow".into();
        doc.lights.push(glow);
        doc.validate().unwrap();
        doc.layers[0].scene_id = "other".into();
        assert!(doc.validate().is_err(), "prop must share the light's scene");
        doc.layers[0].scene_id = "s".into();
        doc.layers[0].content = Content::Rectangle {
            rect: Rect {
                x: 0.0,
                y: 0.0,
                width: 1.0,
                height: 1.0,
            },
            color: black(),
        };
        assert!(doc.validate().is_err(), "glow needs an image layer");
        let extra = r#"{"id":"x","scene_id":"s","source":{"type":"ambient","color":{"r":0,"g":0,"b":0,"a":255},"strength":1,"extra":1}}"#;
        assert!(serde_json::from_str::<Light>(extra).is_err());
    }

    #[test]
    fn animated_layers_report_every_frame_asset_and_commands_work() {
        let mut layer = prop("phone");
        assert_eq!(layer.image_assets().collect::<Vec<_>>(), ["a", "b"]);
        layer.active = Some(TimeRange {
            start: 1.0,
            end: 4.0,
        });
        assert_eq!(layer.animated_asset(1.0), Some("a"));
        assert_eq!(layer.animated_asset(1.5), Some("b"));
        assert_eq!(layer.animated_asset(0.2), Some("a"));
        let mut doc = Document::default();
        doc.apply(crate::Command::Add(layer)).unwrap();
        doc.apply(crate::Command::AddLight(lamp())).unwrap();
        assert!(doc.order.is_empty(), "lights must not materialize order");
        let mut changed = lamp();
        changed.visible = false;
        doc.apply(crate::Command::ReplaceLight(changed.clone()))
            .unwrap();
        assert_eq!(doc.lights[0], changed);
        assert!(doc
            .apply(crate::Command::RemoveLight { id: "none".into() })
            .is_err());
        doc.apply(crate::Command::RemoveLight { id: "lamp".into() })
            .unwrap();
        let json = serde_json::to_string(&doc).unwrap();
        assert!(!json.contains("lights"));
        assert_eq!(serde_json::from_str::<Document>(&json).unwrap(), doc);
    }
}
