//! Onion skin: the characters' poses a few frames (or keys) before and after
//! the playhead, drawn as tinted outlines over the stage. Past poses are red,
//! future poses green, fading with distance. The poses come from the same
//! scene evaluation as playback; the current frame's camera frames them, so
//! movement on screen reads against a still view. UI state only, never saved.
use eframe::egui;
use motion_script::Script;

/// Most ghosts on each side of the playhead.
pub(crate) const MAX_GHOSTS: u8 = 3;
/// Widest frame step between ghosts.
pub(crate) const MAX_STEP: u32 = 12;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct OnionSkin {
    pub enabled: bool,
    pub before: u8,
    pub after: u8,
    /// Frames between ghosts when `keys` is off.
    pub step: u32,
    /// Place ghosts on the authored keys around the playhead instead.
    pub keys: bool,
    /// Ghost only the selected character.
    pub only_selected: bool,
}
impl Default for OnionSkin {
    fn default() -> Self {
        Self {
            enabled: false,
            before: 2,
            after: 2,
            step: 2,
            keys: false,
            only_selected: false,
        }
    }
}

/// One ghost: its signed distance from the playhead (−1 is the nearest past
/// ghost) and the outlines of its characters in scene space.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Ghost {
    pub offset: i32,
    pub time: f64,
    pub outlines: Vec<Vec<[f64; 2]>>,
}

/// Every authored key time of `actor` in the scene (root, bone and joint tracks).
pub(crate) fn key_times(script: &Script, scene_index: usize, actor: Option<&str>) -> Vec<f64> {
    let Some(scene) = script.scenes.get(scene_index) else {
        return Vec::new();
    };
    let composition = &scene.composition;
    let wanted = |id: &String| actor.is_none_or(|a| a == id);
    let mut times: Vec<f64> = composition
        .actor_tracks
        .iter()
        .filter(|(id, _)| wanted(id))
        .flat_map(|(_, keys)| keys.iter().map(|k| k.at))
        .chain(
            composition
                .bone_tracks
                .iter()
                .chain(&composition.joint_tracks)
                .filter(|(id, _)| wanted(id))
                .flat_map(|(_, tracks)| tracks.values().flatten().map(|k| k.at)),
        )
        .filter(|t| t.is_finite())
        .collect();
    times.sort_by(f64::total_cmp);
    times.dedup_by(|a, b| (*a - *b).abs() < 1e-9);
    times
}

impl OnionSkin {
    /// Ghost times around `playhead`: `(time, offset)`, nearest first on each
    /// side, inside `0..duration`. In key mode they are the keys strictly
    /// before and after the playhead (more than half a frame away).
    pub(crate) fn times(
        &self,
        playhead: f64,
        fps: u32,
        duration: f64,
        keys: &[f64],
    ) -> Vec<(f64, i32)> {
        if !self.enabled || !playhead.is_finite() {
            return Vec::new();
        }
        let frame = 1.0 / f64::from(fps.max(1));
        let before = self.before.min(MAX_GHOSTS) as usize;
        let after = self.after.min(MAX_GHOSTS) as usize;
        let mut out = Vec::new();
        if self.keys {
            let past = keys.iter().rev().filter(|t| **t < playhead - frame / 2.0);
            for (i, t) in past.take(before).enumerate() {
                out.push((*t, -(i as i32) - 1));
            }
            let future = keys.iter().filter(|t| **t > playhead + frame / 2.0);
            for (i, t) in future.take(after).enumerate() {
                out.push((*t, i as i32 + 1));
            }
        } else {
            let step = f64::from(self.step.clamp(1, MAX_STEP)) * frame;
            for i in 1..=before {
                let t = playhead - step * i as f64;
                if t >= 0.0 {
                    out.push((t, -(i as i32)));
                }
            }
            for i in 1..=after {
                let t = playhead + step * i as f64;
                if t < duration {
                    out.push((t, i as i32));
                }
            }
        }
        out
    }

    /// Settings row; returns true when anything changed.
    pub(crate) fn ui(&mut self, ui: &mut egui::Ui) -> bool {
        let before = self.clone();
        ui.horizontal_wrapped(|ui| {
            ui.checkbox(&mut self.enabled, "Onion skin").on_hover_text(
                "Näyttää hahmojen asennot ennen (punainen) ja jälkeen (vihreä) toistopään.",
            );
            ui.add_enabled_ui(self.enabled, |ui| {
                ui.label("ennen");
                ui.add(egui::DragValue::new(&mut self.before).range(0..=MAX_GHOSTS));
                ui.label("jälkeen");
                ui.add(egui::DragValue::new(&mut self.after).range(0..=MAX_GHOSTS));
                ui.selectable_value(&mut self.keys, false, "ruuduittain");
                ui.selectable_value(&mut self.keys, true, "avainruuduittain");
                if !self.keys {
                    ui.label("väli");
                    ui.add(
                        egui::DragValue::new(&mut self.step)
                            .range(1..=MAX_STEP)
                            .suffix(" ruutua"),
                    );
                }
                ui.checkbox(&mut self.only_selected, "vain valittu hahmo");
            });
        });
        *self != before
    }
}

/// Evaluate each ghost time and collect the outlines of its characters
/// (`only`: one character). Times that fail to evaluate are skipped.
pub(crate) fn ghosts(
    script: &Script,
    scene_index: usize,
    times: &[(f64, i32)],
    only: Option<&str>,
) -> Vec<Ghost> {
    times
        .iter()
        .filter_map(|&(time, offset)| {
            let scene = motion_scene::evaluate(script, scene_index, time).ok()?;
            let outlines = scene
                .actors
                .iter()
                .filter(|actor| only.is_none_or(|id| id == actor.id))
                .flat_map(actor_outlines)
                .collect();
            Some(Ghost {
                offset,
                time,
                outlines,
            })
        })
        .collect()
}

/// Scene-space outlines of one pose: its rigid parts, or for the built-in
/// figure its bones as two-point lines.
fn actor_outlines(actor: &motion_scene::ActorPose) -> Vec<Vec<[f64; 2]>> {
    let world = |p: motion_scene::Vec2| {
        let w = actor.local_to_world(p);
        [w.x, w.y]
    };
    if !actor.rigid_parts.is_empty() {
        return actor
            .rigid_parts
            .iter()
            .filter(|part| part.color[3] > 0 && part.points.len() >= 2)
            .map(|part| part.points.iter().map(|p| world(*p)).collect())
            .collect();
    }
    use motion_scene::{CHEST, HEAD, HIP, LEFT_FOOT, LEFT_HAND, RIGHT_FOOT, RIGHT_HAND};
    let s = &actor.skeleton;
    [
        (HIP, CHEST),
        (CHEST, HEAD),
        (CHEST, LEFT_HAND),
        (CHEST, RIGHT_HAND),
        (HIP, LEFT_FOOT),
        (HIP, RIGHT_FOOT),
    ]
    .iter()
    .map(|(a, b)| vec![world(s[*a]), world(s[*b])])
    .collect()
}

/// Tint of a ghost: red in the past, green in the future, fainter with distance.
fn tint(offset: i32) -> egui::Color32 {
    let alpha = match offset.unsigned_abs() {
        1 => 170,
        2 => 110,
        _ => 70,
    };
    if offset < 0 {
        egui::Color32::from_rgba_unmultiplied(220, 70, 60, alpha)
    } else {
        egui::Color32::from_rgba_unmultiplied(50, 170, 90, alpha)
    }
}

/// Draw the ghosts over the stage `rect`, framed by the current `scene`'s camera.
pub(crate) fn paint(
    painter: &egui::Painter,
    rect: egui::Rect,
    scene: &motion_scene::EvaluatedScene,
    ghosts: &[Ghost],
) {
    // Farthest first, so the nearest ghosts stay on top.
    let mut order: Vec<&Ghost> = ghosts.iter().collect();
    order.sort_by_key(|g| std::cmp::Reverse(g.offset.unsigned_abs()));
    for ghost in order {
        let stroke = egui::Stroke::new(1.5_f32, tint(ghost.offset));
        for outline in &ghost.outlines {
            let points: Vec<egui::Pos2> = outline
                .iter()
                .map(|p| crate::direct_manipulation::screen_point(rect, scene, p[0], p[1]))
                .collect();
            if points.len() == 2 {
                painter.line_segment([points[0], points[1]], stroke);
            } else {
                painter.add(egui::Shape::closed_line(points, stroke));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn on() -> OnionSkin {
        OnionSkin {
            enabled: true,
            ..Default::default()
        }
    }

    #[test]
    fn frame_ghosts_step_both_ways_inside_the_scene() {
        let skin = on();
        let times = skin.times(1.0, 24, 2.0, &[]);
        let expected = [
            (1.0 - 2.0 / 24.0, -1),
            (1.0 - 4.0 / 24.0, -2),
            (1.0 + 2.0 / 24.0, 1),
            (1.0 + 4.0 / 24.0, 2),
        ];
        assert_eq!(times.len(), 4);
        for ((t, o), (et, eo)) in times.iter().zip(expected) {
            assert!((t - et).abs() < 1e-12);
            assert_eq!(*o, eo);
        }
        // Near the start, past ghosts before 0 are dropped.
        assert_eq!(
            skin.times(0.05, 24, 2.0, &[])
                .iter()
                .filter(|g| g.1 < 0)
                .count(),
            0
        );
        assert!(OnionSkin::default().times(1.0, 24, 2.0, &[]).is_empty());
        let wide = OnionSkin {
            before: 9,
            step: 99,
            ..on()
        };
        assert_eq!(
            wide.times(3.0, 24, 4.0, &[])
                .iter()
                .filter(|g| g.1 < 0)
                .count(),
            3
        );
    }

    #[test]
    fn key_ghosts_use_the_nearest_keys_and_skip_the_current_one() {
        let skin = OnionSkin {
            keys: true,
            before: 1,
            ..on()
        };
        let times = skin.times(1.0, 24, 5.0, &[0.0, 0.5, 1.0, 2.0, 3.0, 4.0]);
        assert_eq!(times, [(0.5, -1), (2.0, 1), (3.0, 2)]);
    }

    #[test]
    fn ghosts_follow_the_evaluated_motion_and_collect_keys() {
        let script = motion_script::parse(
            "characters: {A: {asset: a}, B: {asset: b}}\nscenes: [{id: s, duration: 4, events: [], composition: {actor_tracks: {A: [{at: 0, x: 0}, {at: 2, x: 200}], B: [{at: 1, y: 5}]}}}]\n",
        )
        .unwrap();
        assert_eq!(key_times(&script, 0, None), [0.0, 1.0, 2.0]);
        assert_eq!(key_times(&script, 0, Some("A")), [0.0, 2.0]);
        let ghosts = ghosts(&script, 0, &[(0.0, -1), (2.0, 1)], Some("A"));
        assert_eq!(ghosts.len(), 2);
        // Built-in figure: six bones, each a two-point line, 200 px apart in time.
        assert_eq!(ghosts[0].outlines.len(), 6);
        let dx = ghosts[1].outlines[0][0][0] - ghosts[0].outlines[0][0][0];
        assert!((dx - 200.0).abs() < 1e-9, "{dx}");
        let both = super::ghosts(&script, 0, &[(0.0, -1)], None);
        assert_eq!(both[0].outlines.len(), 12);
        assert!(tint(-1).r() > tint(-1).g() && tint(1).g() > tint(1).r());
        assert!(tint(3).a() < tint(1).a());
    }

    #[test]
    fn settings_report_changes_headlessly() {
        let mut skin = OnionSkin::default();
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(!skin.ui(ui));
            });
        });
    }
}
