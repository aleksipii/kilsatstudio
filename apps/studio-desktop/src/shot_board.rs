//! Shots and their production status: the shot list of the open screenplay
//! with a status and a note per shot. The studio owns the document; this
//! panel only edits it.
use eframe::egui;
use motion_project::{ProductionDocument, ShotStatus};
use std::collections::BTreeMap;

#[derive(Default)]
pub(crate) struct ShotBoard {
    /// Notes being typed, committed when the field loses focus.
    drafts: BTreeMap<String, String>,
    /// Show only shots at this status.
    filter: Option<ShotStatus>,
}

/// One shot as a short line: duration, framing and the first words spoken.
fn summary(shot: &motion_script::ShotInfo) -> String {
    let size = match shot.size {
        motion_script::ShotSize::Wide => "laaja",
        motion_script::ShotSize::Medium => "keski",
        motion_script::ShotSize::CloseUp => "lähi",
    };
    let line = shot.lines.first().map_or(String::new(), |l| {
        let words: String = l.text.chars().take(40).collect();
        let more = if l.text.chars().count() > 40 {
            "…"
        } else {
            ""
        };
        format!(" · {}: {words}{more}", l.actor)
    });
    format!("{:.1} s · {size}{line}", shot.duration()).replace('.', ",")
}

impl ShotBoard {
    /// Draw the board. Returns a status message when a record changed.
    pub(crate) fn ui(
        &mut self,
        ui: &mut egui::Ui,
        script: Option<&motion_script::Script>,
        production: &mut ProductionDocument,
        enabled: bool,
    ) -> Option<String> {
        let mut message = None;
        egui::CollapsingHeader::new("Otokset ja tilat").show(ui, |ui| {
            let Some(script) = script else {
                ui.weak("Avaa tai luo käsikirjoitus nähdäksesi sen otokset.");
                return;
            };
            let shots = match motion_script::shot_list(script) {
                Ok(shots) => shots,
                Err(error) => {
                    ui.label(format!("Otoslistaa ei voi muodostaa: {error}"));
                    return;
                }
            };
            ui.weak("Otos alkaa jokaisesta kohtauksesta, kovasta leikkauksesta ja kamerakulman vaihdosta. Tila ja muistiinpano tallentuvat .kilsat-projektiin; ne eivät kuulu kumoa-historiaan.");
            let counts = production.summary(&shots);
            ui.label(
                counts
                    .iter()
                    .map(|(status, n)| format!("{} {n}", status.label()))
                    .collect::<Vec<_>>()
                    .join(" · "),
            );
            egui::ComboBox::from_label("Näytä")
                .selected_text(self.filter.map_or("Kaikki otokset", ShotStatus::label))
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.filter, None, "Kaikki otokset");
                    for status in ShotStatus::ALL {
                        ui.selectable_value(&mut self.filter, Some(status), status.label());
                    }
                });
            let mut change: Option<(usize, ShotStatus, String)> = None;
            let tracked: Vec<_> = production
                .track(&shots)
                .into_iter()
                .map(|t| {
                    (
                        t.record.map_or(ShotStatus::Storyboard, |r| r.status),
                        t.record.map_or(String::new(), |r| r.note.clone()),
                        t.moved,
                    )
                })
                .collect();
            ui.add_enabled_ui(enabled, |ui| {
                egui::ScrollArea::vertical()
                    .max_height(360.0)
                    .id_salt("shot_board")
                    .show(ui, |ui| {
                        for (index, (shot, (status, note, moved))) in
                            shots.iter().zip(&tracked).enumerate()
                        {
                            if self.filter.is_some_and(|f| f != *status) {
                                continue;
                            }
                            ui.horizontal(|ui| {
                                ui.strong(&shot.id);
                                let mut chosen = *status;
                                egui::ComboBox::from_id_salt(("shot_status", index))
                                    .selected_text(chosen.label())
                                    .show_ui(ui, |ui| {
                                        for option in ShotStatus::ALL {
                                            ui.selectable_value(&mut chosen, option, option.label());
                                        }
                                    });
                                if chosen != *status {
                                    change = Some((index, chosen, note.clone()));
                                }
                            });
                            ui.weak(summary(shot));
                            if *moved {
                                ui.colored_label(
                                    ui.visuals().warn_fg_color,
                                    "Otos on siirtynyt merkinnän jälkeen – tarkista tila.",
                                );
                            }
                            let draft = self
                                .drafts
                                .entry(shot.id.clone())
                                .or_insert_with(|| note.clone());
                            let field = ui.add(
                                egui::TextEdit::singleline(draft)
                                    .hint_text("Muistiinpano")
                                    .char_limit(motion_project::production::MAX_SHOT_NOTE_CHARS),
                            );
                            if field.lost_focus() && draft != note {
                                change = Some((index, *status, draft.clone()));
                            }
                            ui.separator();
                        }
                    });
            });
            if let Some((index, status, note)) = change {
                let shot = &shots[index];
                message = Some(match production.set(shot, status, &note) {
                    Ok(()) => {
                        self.drafts.insert(shot.id.clone(), note);
                        format!(
                            "Otos {}: {}. Tallenna projekti säilyttääksesi tilan.",
                            shot.id,
                            status.label()
                        )
                    }
                    Err(error) => format!("Otoksen {} tila: {error}", shot.id),
                });
            }
            let orphans: Vec<String> = production
                .orphans(&shots)
                .into_iter()
                .map(str::to_string)
                .collect();
            if !orphans.is_empty() {
                ui.label(format!(
                    "Merkinnät, joiden otosta ei enää ole: {}",
                    orphans.join(", ")
                ));
                if ui
                    .add_enabled(enabled, egui::Button::new("Poista vanhat merkinnät"))
                    .clicked()
                {
                    for id in &orphans {
                        production.shots.remove(id);
                        self.drafts.remove(id);
                    }
                    message = Some(format!("{} vanhaa merkintää poistettu.", orphans.len()));
                }
            }
        });
        message
    }
    /// Forget typed notes (another project was opened).
    pub(crate) fn reset(&mut self) {
        self.drafts.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summaries_use_finnish_decimals_and_shorten_long_lines() {
        let script = motion_script::parse(
            "characters: {A: {asset: a, voice: {provider: kokoro, preset: af_heart, language: en-us, speed: 1}}}\nscenes: [{id: s, duration: 2.5, events: [{type: speak, at: 0.5, actor: A, text: 'Tämä on hyvin pitkä repliikki, joka ei mahdu yhdelle riville'}]}]\n",
        )
        .unwrap();
        let shots = motion_script::shot_list(&script).unwrap();
        let text = summary(&shots[0]);
        assert!(
            text.starts_with("2,5 s · laaja · A: Tämä on hyvin"),
            "{text}"
        );
        assert!(text.ends_with('…'));
    }

    #[test]
    fn the_board_lists_shots_and_records_a_status_headlessly() {
        let script =
            motion_script::parse("characters: {}\nscenes: [{id: s, duration: 2, events: []}]\n")
                .unwrap();
        let mut production = ProductionDocument::default();
        let mut board = ShotBoard::default();
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(board.ui(ui, Some(&script), &mut production, true).is_none());
                assert!(board.ui(ui, None, &mut production, true).is_none());
            });
        });
        let shots = motion_script::shot_list(&script).unwrap();
        production.set(&shots[0], ShotStatus::Done, "ok").unwrap();
        assert_eq!(production.summary(&shots)[&ShotStatus::Done], 1);
    }
}
