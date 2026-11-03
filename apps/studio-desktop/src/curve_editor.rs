//! Graph editor for a track of `ActorKey`s: value curves over time per
//! channel, draggable keys (time snaps to frames), multi-selection, inserting
//! keys on the curve, and a timing-curve box with two Bézier handles for the
//! selected segment. Curves are drawn with the engine's own track sampling, so
//! the graph shows exactly what plays. Editing changes only the caller's draft;
//! the caller validates and applies it.
use eframe::egui;
use motion_script::{ActorEasing, ActorKey};
use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    X,
    Y,
    Rotation,
    ScaleX,
    ScaleY,
}
impl Channel {
    pub const ALL: [Channel; 5] = [
        Channel::X,
        Channel::Y,
        Channel::Rotation,
        Channel::ScaleX,
        Channel::ScaleY,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Channel::X => "X",
            Channel::Y => "Y",
            Channel::Rotation => "Kierto",
            Channel::ScaleX => "X-mittakaava",
            Channel::ScaleY => "Y-mittakaava",
        }
    }
    fn color(self) -> egui::Color32 {
        match self {
            Channel::X => egui::Color32::from_rgb(226, 86, 76),
            Channel::Y => egui::Color32::from_rgb(88, 178, 92),
            Channel::Rotation => egui::Color32::from_rgb(80, 140, 230),
            Channel::ScaleX => egui::Color32::from_rgb(214, 160, 50),
            Channel::ScaleY => egui::Color32::from_rgb(170, 100, 210),
        }
    }
    pub fn get(self, key: &ActorKey) -> f64 {
        match self {
            Channel::X => key.x,
            Channel::Y => key.y,
            Channel::Rotation => key.rotation,
            Channel::ScaleX => key.scale_x,
            Channel::ScaleY => key.scale_y,
        }
    }
    pub fn set(self, key: &mut ActorKey, value: f64) {
        match self {
            Channel::X => key.x = value,
            Channel::Y => key.y = value,
            Channel::Rotation => key.rotation = value,
            Channel::ScaleX => key.scale_x = value,
            Channel::ScaleY => key.scale_y = value,
        }
    }
    fn of(self, value: motion_animation::Transform2) -> f64 {
        match self {
            Channel::X => value.translation.x,
            Channel::Y => value.translation.y,
            Channel::Rotation => value.rotation,
            Channel::ScaleX => value.scale.x,
            Channel::ScaleY => value.scale.y,
        }
    }
}

/// The engine track for `keys`, or `None` while the draft is not a valid track.
pub fn track(keys: &[ActorKey]) -> Option<motion_animation::Track> {
    motion_animation::Track::new(keys.iter().map(ActorKey::animation_key).collect()).ok()
}

/// A channel's value at `time`, sampled exactly as playback samples it.
pub fn sample(track: &motion_animation::Track, channel: Channel, time: f64) -> Option<f64> {
    track.sample(time).ok().map(|value| channel.of(value))
}

/// Starting Bézier handles that resemble `easing`, for editing its curve.
pub fn bezier_handles(easing: ActorEasing) -> [f64; 4] {
    match easing {
        ActorEasing::Bezier(x1, y1, x2, y2) => [x1, y1, x2, y2],
        ActorEasing::Linear => [1.0 / 3.0, 1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0],
        ActorEasing::Smoothstep => [0.33, 0.0, 0.67, 1.0],
        ActorEasing::EaseIn => [0.32, 0.0, 0.67, 0.0],
        ActorEasing::EaseOut => [0.33, 1.0, 0.68, 1.0],
        ActorEasing::EaseInOut => [0.65, 0.0, 0.35, 1.0],
        ActorEasing::Back(_) => [0.34, 1.56, 0.64, 1.0],
        ActorEasing::Hold => [0.33, 0.0, 1.0, 0.0],
    }
}

/// Move the `selected` keys of `original` by `dt` seconds (snapped to frames)
/// and `dv` on `channel`. `None` when the move would reorder keys, bring two
/// keys closer than one frame or leave `0..=duration`; the caller then keeps
/// the previous state.
pub fn moved_keys(
    original: &[ActorKey],
    selected: &BTreeSet<usize>,
    dt: f64,
    dv: f64,
    channel: Channel,
    duration: f64,
    fps: u32,
) -> Option<Vec<ActorKey>> {
    let frame = 1.0 / f64::from(fps.max(1));
    let dt = (dt / frame).round() * frame;
    let mut keys = original.to_vec();
    for &index in selected {
        let key = keys.get_mut(index)?;
        key.at = ((key.at + dt) / frame).round() * frame;
        channel.set(key, channel.get(key) + dv);
    }
    let in_scene = keys.iter().all(|k| (0.0..=duration + 1e-9).contains(&k.at));
    let ordered = keys
        .windows(2)
        .all(|pair| pair[1].at - pair[0].at >= frame - 1e-9);
    (in_scene && ordered).then_some(keys)
}

/// Insert a key at `time` (snapped to a frame) with every channel sampled from
/// the current curves and the easing of the key before it. Returns its index,
/// or `None` when a key already sits within a frame of that time.
pub fn insert_key(keys: &mut Vec<ActorKey>, time: f64, fps: u32) -> Option<usize> {
    let frame = 1.0 / f64::from(fps.max(1));
    let at = (time / frame).round() * frame;
    if keys.iter().any(|k| (k.at - at).abs() < frame - 1e-9) || keys.len() >= 4096 || at < 0.0 {
        return None;
    }
    let index = keys.partition_point(|k| k.at < at);
    let mut key = ActorKey {
        at,
        ..keys
            .get(index.saturating_sub(1))
            .or(keys.first())
            .cloned()
            .unwrap_or_default()
    };
    if let Some(track) = track(keys) {
        for channel in Channel::ALL {
            if let Some(value) = sample(&track, channel, at) {
                channel.set(&mut key, value);
            }
        }
    }
    keys.insert(index, key);
    Some(index)
}

/// Lowest and highest value of the shown channels over the keys and the
/// sampled curves, padded; a flat range opens to ±1 around its value.
fn value_range(keys: &[ActorKey], channels: &[Channel], duration: f64) -> (f64, f64) {
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    let mut add = |v: f64| {
        if v.is_finite() {
            lo = lo.min(v);
            hi = hi.max(v);
        }
    };
    for channel in channels {
        for key in keys {
            add(channel.get(key));
        }
    }
    if let Some(track) = track(keys) {
        for i in 0..=200 {
            let t = duration * f64::from(i) / 200.0;
            for channel in channels {
                if let Some(v) = sample(&track, *channel, t) {
                    add(v);
                }
            }
        }
    }
    if !lo.is_finite() {
        return (-1.0, 1.0);
    }
    if hi - lo < 1e-6 {
        return (lo - 1.0, hi + 1.0);
    }
    let pad = (hi - lo) * 0.1;
    (lo - pad, hi + pad)
}

#[derive(Debug, Clone)]
struct Drag {
    original: Vec<ActorKey>,
    start: egui::Pos2,
}

pub struct CurveEditor {
    pub shown: BTreeSet<usize>,
    /// The channel key drags change vertically.
    pub active: Channel,
    pub selected: BTreeSet<usize>,
    /// Key index whose outgoing segment the timing box edits.
    pub segment: Option<usize>,
    drag: Option<Drag>,
}
impl Default for CurveEditor {
    fn default() -> Self {
        Self {
            shown: [0, 1, 2].into_iter().collect(),
            active: Channel::X,
            selected: BTreeSet::new(),
            segment: None,
            drag: None,
        }
    }
}

/// Screen mapping of the graph.
#[derive(Clone, Copy)]
struct View {
    rect: egui::Rect,
    duration: f64,
    lo: f64,
    hi: f64,
}
impl View {
    fn x(&self, t: f64) -> f32 {
        self.rect.left() + (t / self.duration) as f32 * self.rect.width()
    }
    fn y(&self, v: f64) -> f32 {
        self.rect.bottom() - ((v - self.lo) / (self.hi - self.lo)) as f32 * self.rect.height()
    }
    fn time(&self, x: f32) -> f64 {
        f64::from((x - self.rect.left()) / self.rect.width()) * self.duration
    }
    fn per_pixel(&self) -> (f64, f64) {
        (
            self.duration / f64::from(self.rect.width()),
            (self.hi - self.lo) / f64::from(self.rect.height()),
        )
    }
}

impl CurveEditor {
    fn channels(&self) -> Vec<Channel> {
        Channel::ALL
            .iter()
            .enumerate()
            .filter(|(i, _)| self.shown.contains(i))
            .map(|(_, c)| *c)
            .collect()
    }

    /// Draw the editor; returns true when `keys` changed.
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        keys: &mut Vec<ActorKey>,
        duration: f64,
        fps: u32,
        playhead: Option<f64>,
    ) -> bool {
        let mut changed = false;
        self.selected.retain(|i| *i < keys.len());
        if self.segment.is_some_and(|s| s + 1 >= keys.len()) {
            self.segment = None;
        }
        ui.horizontal_wrapped(|ui| {
            ui.label("Käyrät:");
            for (i, channel) in Channel::ALL.iter().enumerate() {
                let mut on = self.shown.contains(&i);
                if ui
                    .checkbox(
                        &mut on,
                        egui::RichText::new(channel.label()).color(channel.color()),
                    )
                    .changed()
                {
                    if on {
                        self.shown.insert(i);
                    } else {
                        self.shown.remove(&i);
                    }
                }
            }
        });
        ui.horizontal(|ui| {
            ui.label("Pystyveto muuttaa:");
            egui::ComboBox::from_id_salt("curve_active")
                .selected_text(self.active.label())
                .show_ui(ui, |ui| {
                    for channel in Channel::ALL {
                        ui.selectable_value(&mut self.active, channel, channel.label());
                    }
                });
        });
        let channels = self.channels();
        let (lo, hi) = value_range(keys, &channels, duration.max(1e-6));
        let width = ui.available_width().max(200.0);
        let (response, painter) =
            ui.allocate_painter(egui::vec2(width, 220.0), egui::Sense::click_and_drag());
        let view = View {
            rect: response.rect.shrink2(egui::vec2(8.0, 10.0)),
            duration: duration.max(1e-6),
            lo,
            hi,
        };
        let visuals = ui.visuals().clone();
        painter.rect_filled(response.rect, 4.0, visuals.extreme_bg_color);
        // Seconds grid and zero line.
        let grid = egui::Stroke::new(1.0_f32, visuals.widgets.noninteractive.bg_stroke.color);
        let mut second = 0.0;
        while second <= duration {
            let x = view.x(second);
            painter.line_segment(
                [
                    egui::pos2(x, view.rect.top()),
                    egui::pos2(x, view.rect.bottom()),
                ],
                grid,
            );
            painter.text(
                egui::pos2(x + 2.0, view.rect.bottom()),
                egui::Align2::LEFT_BOTTOM,
                format!("{second:.0} s"),
                egui::FontId::proportional(10.0),
                visuals.weak_text_color(),
            );
            second += 1.0;
        }
        if lo < 0.0 && hi > 0.0 {
            let y = view.y(0.0);
            painter.line_segment(
                [
                    egui::pos2(view.rect.left(), y),
                    egui::pos2(view.rect.right(), y),
                ],
                grid,
            );
        }
        // Curves, one point per pixel from the engine's own sampling.
        let curve = track(keys);
        if let Some(track) = &curve {
            for channel in &channels {
                let steps = view.rect.width().max(2.0) as usize;
                let points: Vec<egui::Pos2> = (0..=steps)
                    .filter_map(|i| {
                        let t = view.duration * i as f64 / steps as f64;
                        sample(track, *channel, t).map(|v| egui::pos2(view.x(t), view.y(v)))
                    })
                    .collect();
                let stroke_width = if *channel == self.active {
                    2.0_f32
                } else {
                    1.2
                };
                painter.add(egui::Shape::line(
                    points,
                    egui::Stroke::new(stroke_width, channel.color()),
                ));
            }
        } else if !keys.is_empty() {
            painter.text(
                view.rect.center(),
                egui::Align2::CENTER_CENTER,
                "Avainruutujen ajat eivät kasva – korjaa arvot alla.",
                egui::FontId::proportional(13.0),
                visuals.warn_fg_color,
            );
        }
        if let Some(segment) = self.segment {
            let (a, b) = (keys[segment].at, keys[segment + 1].at);
            painter.rect_filled(
                egui::Rect::from_x_y_ranges(view.x(a)..=view.x(b), view.rect.y_range()),
                0.0,
                visuals.selection.bg_fill.gamma_multiply(0.25),
            );
        }
        if let Some(t) = playhead.filter(|t| t.is_finite()) {
            let x = view.x(t);
            painter.line_segment(
                [
                    egui::pos2(x, view.rect.top()),
                    egui::pos2(x, view.rect.bottom()),
                ],
                egui::Stroke::new(1.5_f32, visuals.warn_fg_color),
            );
        }
        // Keys on every shown channel; the active channel's are the handles.
        let pointer = response.interact_pointer_pos().or(response.hover_pos());
        let mut hovered_key = None;
        for (index, key) in keys.iter().enumerate() {
            for channel in &channels {
                let pos = egui::pos2(view.x(key.at), view.y(channel.get(key)));
                let selected = self.selected.contains(&index);
                let radius = if *channel == self.active { 5.0 } else { 3.0 };
                if *channel == self.active && pointer.is_some_and(|p| p.distance(pos) <= 7.0) {
                    hovered_key = Some(index);
                }
                painter.circle(
                    pos,
                    radius,
                    if selected {
                        visuals.selection.bg_fill
                    } else {
                        channel.color()
                    },
                    egui::Stroke::new(1.0_f32, visuals.strong_text_color()),
                );
            }
        }
        let shift = ui.input(|i| i.modifiers.shift);
        if response.drag_started() {
            if let Some(index) = hovered_key {
                if !self.selected.contains(&index) {
                    if !shift {
                        self.selected.clear();
                    }
                    self.selected.insert(index);
                }
                self.drag = Some(Drag {
                    original: keys.clone(),
                    start: response.interact_pointer_pos().unwrap_or_default(),
                });
            }
        }
        if let (Some(drag), Some(now)) = (&self.drag, response.interact_pointer_pos()) {
            if response.dragged() {
                let (per_x, per_y) = view.per_pixel();
                let delta = now - drag.start;
                if let Some(next) = moved_keys(
                    &drag.original,
                    &self.selected,
                    f64::from(delta.x) * per_x,
                    -f64::from(delta.y) * per_y,
                    self.active,
                    duration,
                    fps,
                ) {
                    if next != *keys {
                        *keys = next;
                        changed = true;
                    }
                }
            }
        }
        if response.drag_stopped() {
            self.drag = None;
        }
        if response.double_clicked() {
            if let Some(p) = response.interact_pointer_pos() {
                if let Some(index) = insert_key(keys, view.time(p.x), fps) {
                    self.selected = [index].into_iter().collect();
                    self.segment = None;
                    changed = true;
                }
            }
        } else if response.clicked() {
            match (hovered_key, response.interact_pointer_pos()) {
                (Some(index), _) => {
                    if shift {
                        if !self.selected.remove(&index) {
                            self.selected.insert(index);
                        }
                    } else {
                        self.selected = [index].into_iter().collect();
                    }
                }
                (None, Some(p)) => {
                    let t = view.time(p.x);
                    self.selected.clear();
                    self.segment = keys
                        .windows(2)
                        .position(|pair| pair[0].at <= t && t < pair[1].at);
                }
                _ => {}
            }
        }
        ui.small("Vedä avainta: vaaka = aika (ruutuihin), pysty = valittu käyrä. Vaihto+napsautus valitsee useita. Kaksoisnapsautus lisää avaimen käyrälle. Napsauta avainten väliä muokataksesi sen ajoituskäyrää.");
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    !self.selected.is_empty(),
                    egui::Button::new(format!("Poista valitut ({})", self.selected.len())),
                )
                .clicked()
            {
                for index in self.selected.iter().rev() {
                    keys.remove(*index);
                }
                self.selected.clear();
                self.segment = None;
                changed = true;
            }
        });
        if let Some(segment) = self.segment {
            changed |= self.segment_ui(ui, &mut keys[segment]);
        }
        changed
    }

    /// Timing curve of the segment that starts at `key`: presets and a box
    /// with two draggable Bézier handles (x = time, y = progress).
    fn segment_ui(&mut self, ui: &mut egui::Ui, key: &mut ActorKey) -> bool {
        let mut changed = false;
        ui.separator();
        ui.label(format!("Ajoituskäyrä avaimesta {:.2} s eteenpäin", key.at).replace('.', ","));
        ui.horizontal_wrapped(|ui| {
            for (label, easing) in [
                ("Tasainen", ActorEasing::Linear),
                ("Pehmeä", ActorEasing::Smoothstep),
                ("Kiihtyy", ActorEasing::EaseIn),
                ("Hidastuu", ActorEasing::EaseOut),
                ("Kiihtyy ja hidastuu", ActorEasing::EaseInOut),
                (
                    "Ylilyönti",
                    ActorEasing::Back(motion_animation::DEFAULT_BACK_OVERSHOOT),
                ),
                ("Pito", ActorEasing::Hold),
            ] {
                if ui.selectable_label(key.easing == easing, label).clicked() {
                    key.easing = easing;
                    changed = true;
                }
            }
        });
        let [mut x1, mut y1, mut x2, mut y2] = bezier_handles(key.easing);
        let side = 180.0;
        let (response, painter) =
            ui.allocate_painter(egui::vec2(side, side), egui::Sense::click_and_drag());
        let visuals = ui.visuals().clone();
        // Progress -1..2 fits anticipation and overshoot; the unit square is marked.
        let frame = response.rect.shrink(6.0);
        let to_screen = |x: f64, y: f64| {
            egui::pos2(
                frame.left() + x as f32 * frame.width(),
                frame.bottom() - ((y + 1.0) / 3.0) as f32 * frame.height(),
            )
        };
        painter.rect_filled(response.rect, 4.0, visuals.extreme_bg_color);
        painter.rect_stroke(
            egui::Rect::from_two_pos(to_screen(0.0, 0.0), to_screen(1.0, 1.0)),
            0.0,
            egui::Stroke::new(1.0_f32, visuals.widgets.noninteractive.bg_stroke.color),
            egui::StrokeKind::Inside,
        );
        let easing = key.easing.animation_easing();
        let points: Vec<_> = (0..=60)
            .map(|i| {
                let t = f64::from(i) / 60.0;
                to_screen(t, easing.apply(t))
            })
            .collect();
        painter.add(egui::Shape::line(
            points,
            egui::Stroke::new(2.0_f32, visuals.strong_text_color()),
        ));
        let handles = [
            (to_screen(0.0, 0.0), to_screen(x1, y1)),
            (to_screen(1.0, 1.0), to_screen(x2, y2)),
        ];
        for (anchor, handle) in handles {
            painter.line_segment(
                [anchor, handle],
                egui::Stroke::new(1.0_f32, visuals.weak_text_color()),
            );
            painter.circle_filled(handle, 5.0, visuals.selection.bg_fill);
        }
        if let Some(p) = response
            .interact_pointer_pos()
            .filter(|_| response.dragged())
        {
            let x = f64::from((p.x - frame.left()) / frame.width()).clamp(0.0, 1.0);
            let y = (f64::from((frame.bottom() - p.y) / frame.height()) * 3.0 - 1.0).clamp(
                motion_animation::MIN_BEZIER_Y,
                motion_animation::MAX_BEZIER_Y,
            );
            if p.distance(handles[0].1) <= p.distance(handles[1].1) {
                (x1, y1) = (x, y);
            } else {
                (x2, y2) = (x, y);
            }
            key.easing = ActorEasing::Bezier(round(x1), round(y1), round(x2), round(y2));
            changed = true;
        }
        ui.horizontal(|ui| {
            let mut handles = [x1, y1, x2, y2];
            let mut edited = false;
            for (value, label) in handles.iter_mut().zip(["x1", "y1", "x2", "y2"]) {
                let range = if label.starts_with('x') {
                    0.0..=1.0
                } else {
                    motion_animation::MIN_BEZIER_Y..=motion_animation::MAX_BEZIER_Y
                };
                ui.label(label);
                edited |= ui
                    .add(egui::DragValue::new(value).speed(0.01).range(range))
                    .changed();
            }
            if edited {
                key.easing = ActorEasing::Bezier(
                    round(handles[0]),
                    round(handles[1]),
                    round(handles[2]),
                    round(handles[3]),
                );
                changed = true;
            }
        });
        ui.small("Vaaka = aika, pysty = eteneminen. Alle 0 ennakoi, yli 1 menee ohi ja palaa.");
        changed
    }
}

/// Handles are stored to three decimals so the source stays readable.
fn round(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> Vec<ActorKey> {
        vec![
            ActorKey {
                at: 0.0,
                x: 0.0,
                easing: ActorEasing::Bezier(0.42, 0.0, 0.58, 1.0),
                ..Default::default()
            },
            ActorKey {
                at: 1.0,
                x: 100.0,
                rotation: 0.5,
                ..Default::default()
            },
            ActorKey {
                at: 2.0,
                x: 40.0,
                ..Default::default()
            },
        ]
    }

    #[test]
    fn curves_sample_like_the_engine() {
        let keys = keys();
        let track = track(&keys).unwrap();
        // The ease-in-out Bézier is symmetric: half way in time is half way in value.
        assert!((sample(&track, Channel::X, 0.5).unwrap() - 50.0).abs() < 1e-9);
        assert!(sample(&track, Channel::X, 0.1).unwrap() < 10.0);
        let source = format!(
            "characters: {{A: {{asset: a}}}}\nscenes: [{{id: s, duration: 3, events: [], composition: {{actor_tracks: {{A: {}}}}}}}]\n",
            serde_json::to_string(&keys).unwrap()
        );
        let script = motion_script::parse(&source).unwrap();
        let at = |t: f64| motion_scene::evaluate(&script, 0, t).unwrap().actors[0].x;
        let base = at(0.0);
        for t in [0.25, 0.5, 1.5] {
            let expected = sample(&track, Channel::X, t).unwrap();
            assert!((at(t) - base - expected).abs() < 1e-9, "{t}");
        }
    }

    #[test]
    fn dragging_snaps_to_frames_and_never_reorders_keys() {
        let keys = keys();
        let middle: BTreeSet<usize> = [1].into_iter().collect();
        let moved = moved_keys(&keys, &middle, 0.26, 5.0, Channel::X, 3.0, 24).unwrap();
        assert!((moved[1].at - 30.0 / 24.0).abs() < 1e-12);
        assert_eq!(moved[1].x, 105.0);
        assert_eq!(moved[0], keys[0]);
        // Past the next key, or out of the scene: refused.
        assert!(moved_keys(&keys, &middle, 1.0, 0.0, Channel::X, 3.0, 24).is_none());
        let first: BTreeSet<usize> = [0].into_iter().collect();
        assert!(moved_keys(&keys, &first, -0.5, 0.0, Channel::X, 3.0, 24).is_none());
        // Several selected keys move together.
        let both: BTreeSet<usize> = [1, 2].into_iter().collect();
        let moved = moved_keys(&keys, &both, 0.5, 0.0, Channel::Rotation, 3.0, 24).unwrap();
        assert_eq!((moved[1].at, moved[2].at), (1.5, 2.5));
        assert!(moved_keys(&keys, &both, 1.5, 0.0, Channel::X, 3.0, 24).is_none());
    }

    #[test]
    fn inserted_keys_keep_the_curve_value_and_frame() {
        let mut keys = keys();
        let before = sample(&track(&keys).unwrap(), Channel::X, 1.5).unwrap();
        let index = insert_key(&mut keys, 1.51, 24).unwrap();
        assert_eq!(index, 2);
        assert_eq!(keys[2].at, 1.5);
        assert!((keys[2].x - before).abs() < 1e-9);
        assert!(insert_key(&mut keys, 1.5, 24).is_none());
        assert!(track(&keys).is_some());
    }

    #[test]
    fn presets_open_as_matching_handles_and_ranges_never_collapse() {
        let linear = bezier_handles(ActorEasing::Linear);
        let easing = ActorEasing::Bezier(linear[0], linear[1], linear[2], linear[3]);
        assert!((easing.apply(0.3) - 0.3).abs() < 1e-9);
        for preset in [
            ActorEasing::EaseInOut,
            ActorEasing::Back(1.7),
            ActorEasing::Hold,
        ] {
            let [x1, y1, x2, y2] = bezier_handles(preset);
            assert!(ActorEasing::Bezier(x1, y1, x2, y2).is_valid(), "{preset:?}");
        }
        let flat = vec![ActorKey::default()];
        assert_eq!(value_range(&flat, &[Channel::X], 1.0), (-1.0, 1.0));
        assert_eq!(value_range(&[], &[Channel::X], 1.0), (-1.0, 1.0));
    }

    #[test]
    fn the_editor_draws_headlessly_and_reports_no_change() {
        let mut keys = keys();
        let mut editor = CurveEditor {
            segment: Some(0),
            ..Default::default()
        };
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!editor.ui(ui, &mut keys, 3.0, 24, Some(1.2)));
            });
        });
        assert_eq!(keys, self::keys());
    }
}
