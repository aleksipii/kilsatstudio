//! Detached composition controls; Apply is validated as one screenplay edit.
use eframe::egui;
use motion_script::{
    camera_preset_keys, BackgroundKey, CameraKey, CameraPreset, Composition, Layer, Transition,
};

pub fn ui(
    ui: &mut egui::Ui,
    value: &mut Composition,
    actors: &[String],
    duration: f64,
    is_last_scene: bool,
) {
    outgoing_transition_ui(ui, &mut value.transition, is_last_scene);
    ui.separator();
    ui.label("Taustaväri (#RRGGBB)");
    ui.add(egui::TextEdit::singleline(&mut value.background_color).char_limit(7));
    ui.separator();
    ui.label("Ajoitetut taustavärit");
    ui.small("Perusväri näkyy ennen ensimmäistä avainruutua. Väri vaihtuu avainruudun hetkellä ja säilyy seuraavaan.");
    let mut remove_background = None;
    for (index, key) in value.background_keys.iter_mut().enumerate() {
        ui.push_id(("background", index), |ui| {
            ui.horizontal(|ui| {
                ui.label(format!("Avainruutu {} · sekuntia", index + 1));
                ui.add(egui::DragValue::new(&mut key.at).speed(0.1));
                if ui.small_button("Poista avainruutu").clicked() {
                    remove_background = Some(index);
                }
            });
            ui.horizontal(|ui| {
                ui.label("Väri");
                ui.add(
                    egui::TextEdit::singleline(&mut key.color)
                        .char_limit(7)
                        .desired_width(85.0),
                );
                if let Ok(color) = egui::Color32::from_hex(&key.color) {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(18.0, 18.0), egui::Sense::hover());
                    ui.painter().rect_filled(rect, 2.0, color);
                }
            });
        });
    }
    if let Some(index) = remove_background {
        value.background_keys.remove(index);
    }
    // Do not create duplicate times when the last key is already at scene end.
    let next_at = value.background_keys.last().map_or(Some(0.0), |key| {
        if key.at.is_finite() && key.at < duration {
            let candidate = (key.at + 1.0).min(duration);
            (candidate > key.at).then_some(candidate)
        } else {
            None
        }
    });
    if ui
        .add_enabled(
            value.background_keys.len() < 4096 && next_at.is_some(),
            egui::Button::new("Lisää taustavärin avainruutu"),
        )
        .clicked()
    {
        let color = value
            .background_keys
            .last()
            .map(|key| key.color.clone())
            .unwrap_or_else(|| value.background_color.clone());
        value.background_keys.push(BackgroundKey {
            at: next_at.unwrap(),
            color,
        });
    }
    ui.small("Käytä #RRGGBB-värejä ja erillisiä, kasvavia aikoja välillä 0–kohtauksen kesto. Käytä asetuksia tallentaa taustamuutokset yhdessä; Kumoa palauttaa edellisen käsikirjoituksen.");
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("Häivytys sisään / ulos (sekuntia)");
        ui.add(egui::DragValue::new(&mut value.fade_in).speed(0.1));
        ui.add(egui::DragValue::new(&mut value.fade_out).speed(0.1));
    });
    camera_presets_ui(ui, value, duration);
    ui.small("Kamerasiirrot ovat pikseleitä keskipisteestä. Avainruutujen ajat ovat sekunteja.");
    let mut remove = None;
    for (index, key) in value.camera.iter_mut().enumerate() {
        ui.push_id(index, |ui| {
            ui.horizontal(|ui| {
                ui.label("Aika");
                ui.add(egui::DragValue::new(&mut key.at).speed(0.1));
                if ui.small_button("Poista avainruutu").clicked() {
                    remove = Some(index);
                }
            });
            ui.horizontal(|ui| {
                ui.label("X");
                ui.add(egui::DragValue::new(&mut key.x));
                ui.label("Y");
                ui.add(egui::DragValue::new(&mut key.y));
                ui.label("Lähennys");
                ui.add(egui::DragValue::new(&mut key.zoom).speed(0.05));
            });
        });
    }
    if let Some(index) = remove {
        value.camera.remove(index);
    }
    if value.camera.len() < 1024 && ui.button("Lisää kameran avainruutu").clicked() {
        let at = value
            .camera
            .last()
            .map(|k| (k.at + 1.0).min(duration))
            .unwrap_or(0.0);
        value.camera.push(CameraKey {
            at,
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        });
    }
    ui.small("Avainruutujen aikojen pitää olla erillisiä ja kasvavia. Muuta järjestystä muokkaamalla aikoja ennen käyttämistä.");
    for actor in actors {
        let mut layer = value.layers.get(actor).cloned().unwrap_or(Layer {
            z: 0,
            visible: true,
        });
        let mut changed = false;
        ui.horizontal(|ui| {
            changed |= ui.checkbox(&mut layer.visible, actor).changed();
            ui.label("Taso");
            changed |= ui.add(egui::DragValue::new(&mut layer.z)).changed();
        });
        if changed {
            value.layers.insert(actor.clone(), layer);
        }
    }
}

fn outgoing_transition_ui(ui: &mut egui::Ui, transition: &mut Transition, is_last_scene: bool) {
    ui.label("Siirtymä seuraavaan kohtaukseen");
    let crossfade = matches!(transition, Transition::Crossfade { .. });
    ui.horizontal(|ui| {
        if ui.selectable_label(!crossfade, "Leikkaus").clicked() {
            *transition = Transition::Cut;
        }
        if ui
            .add_enabled(
                !is_last_scene,
                egui::Button::new("Ristihäivytys").selected(crossfade),
            )
            .clicked()
            && !crossfade
        {
            *transition = Transition::Crossfade { duration: 0.5 };
        }
    });
    if let Transition::Crossfade { duration } = transition {
        ui.horizontal(|ui| {
            ui.label("Limitys (sekuntia)");
            ui.add(egui::DragValue::new(duration).speed(0.05));
        });
    }
    if is_last_scene {
        ui.small("Viimeisessä kohtauksessa käytetään Leikkaus-siirtymää. Viimeisen kohtauksen ristihäivytystä ei voi ottaa käyttöön.");
    } else {
        ui.small("Ristihäivytys limittää kohtauksen lopun seuraavan alkuun. Keston pitää olla positiivinen ja kumpaakin kohtausta lyhyempi. Saapuva ja lähtevä limitys eivät saa osua päällekkäin.");
    }
    ui.small("Siirtymät näkyvät koko projektin esikatselussa ja PNG-/MP4-koosteviennissä. Valitun kohtauksen esikatselu ja vienti käsittävät vain kyseisen kohtauksen.");
}

/// Edits only the detached composition. The existing Apply settings action owns
/// validation and the single source/history transaction.
fn camera_presets_ui(ui: &mut egui::Ui, value: &mut Composition, duration: f64) {
    let replacing = !value.camera.is_empty();
    ui.collapsing("Kameraliikkeiden esiasetukset", |ui| {
        ui.small("Koko kohtauksen tasainen liike: lähennys 1–1,25 tai kameran keskipisteen siirto 100 pikseliä. Avainruutuja voi muokata alla.");
        if replacing {
            ui.small("Painikkeet korvaavat luonnoksen KAIKKI kameran avainruudut. Muut kohtausasetukset säilyvät. Käytä asetuksia tallentaa muutoksen; Kumoa palauttaa edellisen käsikirjoituksen.");
        } else {
            ui.small("Valitse esiasetus ja tallenna se muiden kohtausasetusten kanssa painamalla Käytä asetuksia.");
        }
        ui.horizontal_wrapped(|ui| {
            for (label, preset) in [
                ("Lähennä", CameraPreset::PushIn),
                ("Loitonna", CameraPreset::PullOut),
                ("Siirrä vasemmalle", CameraPreset::PanLeft),
                ("Siirrä oikealle", CameraPreset::PanRight),
            ] {
                let label = if replacing {
                    format!("Korvaa avainruudut: {label}")
                } else {
                    label.into()
                };
                let clicked = ui
                    .add_enabled(
                        duration.is_finite() && duration > 0.0,
                        egui::Button::new(label),
                    )
                    .clicked();
                if let (true, Ok(keys)) = (clicked, camera_preset_keys(preset, duration)) {
                    value.camera = keys;
                }
            }
        });
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn drawing_controls_does_not_create_layer_overrides() {
        let mut value = Composition::default();
        let before = serde_json::to_value(&value).unwrap();
        let ctx = egui::Context::default();
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |panel| {
                ui(panel, &mut value, &["EMMA".into()], 5.0, true);
            });
        });
        assert_eq!(serde_json::to_value(&value).unwrap(), before);
    }
}
