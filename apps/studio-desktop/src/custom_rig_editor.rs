//! Detached custom rigid-character editor; Apply is an atomic source-history edit.
use eframe::egui;
use motion_rig::{PartTextureDefinition, RigDefinition, VertexWeight};
use motion_script::Script;

/// Pure, YAML-draft transformations used by the texture/skin controls. Each one
/// mutates a parsed [`RigDefinition`]; the draft re-serialises it, so the YAML
/// text stays the single source of truth that Apply validates.
pub mod edit {
    use super::*;
    pub const FULL_UV: [f64; 4] = [0.0, 0.0, 1.0, 1.0];

    fn part_mut(
        rig: &mut RigDefinition,
        part: usize,
    ) -> Result<&mut motion_rig::RigidPartDefinition, String> {
        rig.parts
            .get_mut(part)
            .ok_or_else(|| format!("Osaa #{part} ei ole."))
    }
    /// Bind (or with `None` unbind) a texture asset; an existing UV rectangle is kept.
    pub fn set_texture(
        rig: &mut RigDefinition,
        part: usize,
        asset: Option<&str>,
    ) -> Result<(), String> {
        let part = part_mut(rig, part)?;
        let uv = part.texture.as_ref().map_or(FULL_UV, |t| t.uv);
        part.texture = asset.map(|asset| PartTextureDefinition {
            asset: asset.into(),
            uv,
        });
        Ok(())
    }
    pub fn set_uv(rig: &mut RigDefinition, part: usize, uv: [f64; 4]) -> Result<(), String> {
        let part = part_mut(rig, part)?;
        let texture = part
            .texture
            .as_mut()
            .ok_or("Valitse tekstuurin kuvaresurssi ennen UV-koordinaattien muokkaamista.")?;
        texture.uv = uv;
        Ok(())
    }
    /// Give a rigid part explicit skin data: every vertex fully on the attachment joint.
    pub fn ensure_skin(rig: &mut RigDefinition, part: usize) -> Result<(), String> {
        let part = part_mut(rig, part)?;
        if part.skin.len() != part.points.len() {
            part.skin = part
                .points
                .iter()
                .map(|_| {
                    vec![VertexWeight {
                        joint: part.joint.clone(),
                        weight: 1.0,
                    }]
                })
                .collect();
        }
        Ok(())
    }
    pub fn clear_skin(rig: &mut RigDefinition, part: usize) -> Result<(), String> {
        part_mut(rig, part)?.skin.clear();
        Ok(())
    }
    /// Replace one vertex's influences (one or two), creating rigid skin data first if needed.
    pub fn set_influences(
        rig: &mut RigDefinition,
        part: usize,
        vertex: usize,
        influences: Vec<VertexWeight>,
    ) -> Result<(), String> {
        if !(1..=2).contains(&influences.len()) {
            return Err("Kärki tarvitsee yhden tai kaksi nivelvaikutusta.".into());
        }
        ensure_skin(rig, part)?;
        let slot = part_mut(rig, part)?
            .skin
            .get_mut(vertex)
            .ok_or_else(|| format!("Kärkeä #{vertex} ei ole."))?;
        *slot = influences;
        Ok(())
    }
    /// Scale a vertex's weights so they sum to one. Fails (leaving data untouched)
    /// for non-finite, negative or all-zero weights, which cannot be normalised.
    pub fn normalize_vertex(
        rig: &mut RigDefinition,
        part: usize,
        vertex: usize,
    ) -> Result<(), String> {
        let slot = part_mut(rig, part)?
            .skin
            .get_mut(vertex)
            .ok_or_else(|| format!("Kärjellä #{vertex} ei ole muodonmuutospainoja."))?;
        let sum: f64 = slot.iter().map(|w| w.weight).sum();
        if slot.iter().any(|w| !w.weight.is_finite() || w.weight < 0.0)
            || !sum.is_finite()
            || sum <= 0.0
        {
            return Err(format!(
                "Kärki #{vertex}: painojen tulee olla äärellisiä ja vähintään nolla. Ainakin yhden painon on oltava yli nolla normalisointia varten."
            ));
        }
        for w in slot.iter_mut() {
            w.weight /= sum;
        }
        Ok(())
    }
    /// All-or-nothing: on failure the rig is unchanged.
    pub fn normalize_all(rig: &mut RigDefinition, part: usize) -> Result<(), String> {
        let mut trial = rig.clone();
        let count = part_mut(&mut trial, part)?.skin.len();
        (0..count).try_for_each(|v| normalize_vertex(&mut trial, part, v))?;
        *rig = trial;
        Ok(())
    }
    /// Actionable problems visible before Apply, plus the real rig validation error.
    pub fn issues(rig: &RigDefinition, images: &[String]) -> Vec<String> {
        let mut out = Vec::new();
        for part in &rig.parts {
            if let Some(t) = &part.texture {
                if !images.contains(&t.asset) {
                    out.push(format!(
                        "Osa {}: tekstuurin resurssi '{}' ei ole projektin kuvaresurssi.",
                        part.id, t.asset
                    ));
                }
            }
            for (i, v) in part.skin.iter().enumerate() {
                let sum: f64 = v.iter().map(|w| w.weight).sum();
                if (sum - 1.0).abs() > motion_rig::definition::SKIN_WEIGHT_TOLERANCE {
                    out.push(format!(
                        "Osan {} kärki #{i}: painojen summa on {sum:.6}, odotettu summa on 1. Käytä normalisointia.",
                        part.id
                    ));
                }
            }
        }
        if let Err(error) = rig.validate() {
            out.push(format!("Hahmorakenteen tarkistus: {error}"));
        }
        out
    }
}

pub struct Draft {
    pub actor: String,
    pub yaml: String,
    original: String,
    part: usize,
    vertex: usize,
    notice: String,
}
impl Draft {
    pub fn for_actor(script: &Script, actor: &str) -> Self {
        let yaml = script
            .characters
            .get(actor)
            .and_then(|c| c.rig.as_ref())
            .map(|rig| serde_yaml::to_string(rig).expect("validated serializable rig"))
            .unwrap_or_else(|| TEMPLATE.into());
        Self {
            actor: actor.into(),
            original: yaml.clone(),
            yaml,
            part: 0,
            vertex: 0,
            notice: String::new(),
        }
    }
    pub fn apply(&self, source: &str) -> Result<String, String> {
        if self.yaml.len() > 100_000 {
            return Err("Hahmorakenteen luonnos ylittää muokkaimen 100 kt:n rajan.".into());
        }
        let mut script = motion_script::parse(source).map_err(|e| e.to_string())?;
        let rig = serde_yaml::from_str(&self.yaml).map_err(|e| e.to_string())?;
        let character = script
            .characters
            .get_mut(&self.actor)
            .ok_or("Hahmoa ei enää ole.")?;
        character.rig = Some(rig);
        script.validate().map_err(|e| e.to_string())?;
        serde_yaml::to_string(&script).map_err(|e| e.to_string())
    }
    pub fn remove(&self, source: &str) -> Result<String, String> {
        let mut script = motion_script::parse(source).map_err(|e| e.to_string())?;
        script
            .characters
            .get_mut(&self.actor)
            .ok_or("Hahmoa ei enää ole.")?
            .rig = None;
        script.validate().map_err(|e| e.to_string())?;
        serde_yaml::to_string(&script).map_err(|e| e.to_string())
    }
    /// Mutate the parsed draft with `f` and write it back as YAML; a failure keeps the text.
    fn mutate(&mut self, f: impl FnOnce(&mut RigDefinition) -> Result<(), String>) {
        let result = serde_yaml::from_str::<RigDefinition>(&self.yaml)
            .map_err(|e| e.to_string())
            .and_then(|mut rig| {
                f(&mut rig)?;
                serde_yaml::to_string(&rig).map_err(|e| e.to_string())
            });
        match result {
            Ok(yaml) => {
                self.yaml = yaml;
                self.notice.clear();
            }
            Err(error) => self.notice = error,
        }
    }
    fn texture_skin_ui(&mut self, ui: &mut egui::Ui, images: &[String]) {
        egui::CollapsingHeader::new("Osan tekstuuri ja muodonmuutospainot")
            .default_open(true)
            .show(ui, |ui| {
                let rig = match serde_yaml::from_str::<RigDefinition>(&self.yaml) {
                    Ok(rig) => rig,
                    Err(error) => {
                        ui.colored_label(
                            egui::Color32::LIGHT_RED,
                            format!("Korjaa YAML ennen säätimien käyttöä: {error}"),
                        );
                        return;
                    }
                };
                ui.small("Säätimet päivittävät yllä olevan YAML-luonnoksen ja yhtenäistävät sen muotoilun. Kommentit poistetaan. Projekti muuttuu vasta Käytä-painikkeella.");
                if rig.parts.is_empty() {
                    return;
                }
                self.part = self.part.min(rig.parts.len() - 1);
                egui::ComboBox::from_id_salt("rig_part")
                    .selected_text(format!("Osa: {}", rig.parts[self.part].id))
                    .show_ui(ui, |ui| {
                        for (i, p) in rig.parts.iter().enumerate() {
                            ui.selectable_value(&mut self.part, i, &p.id);
                        }
                    });
                let part = &rig.parts[self.part];
                let part_index = self.part;
                if images.is_empty() && part.texture.is_none() {
                    ui.add_enabled(false, egui::Button::new("Tekstuurin kuvaresurssi"));
                    ui.small("Projektissa ei ole kuvaresursseja. Tuo ensin PNG- tai JPEG-kuva projektin Resurssit-osiossa.");
                } else {
                    let current = part.texture.as_ref().map(|t| t.asset.clone());
                    let mut choice = current.clone();
                    egui::ComboBox::from_id_salt("rig_texture")
                        .selected_text(match &current {
                            Some(a) if images.contains(a) => format!("Tekstuuri: {a}"),
                            Some(a) => format!("Tekstuuri: {a} (puuttuu)"),
                            None => "Tekstuuri: ei valittu".into(),
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut choice, None, "ei valittu");
                            for image in images {
                                ui.selectable_value(&mut choice, Some(image.clone()), image);
                            }
                        });
                    if choice != current {
                        self.mutate(|r| edit::set_texture(r, part_index, choice.as_deref()));
                    }
                }
                if let Some(texture) = &part.texture {
                    let mut uv = texture.uv;
                    ui.horizontal(|ui| {
                        for (label, value) in ["u0", "v0", "u1", "v1"].iter().zip(uv.iter_mut()) {
                            ui.label(*label);
                            ui.add(egui::DragValue::new(value).speed(0.005).range(0.0..=1.0));
                        }
                        if ui.button("Koko kuva").clicked() {
                            uv = edit::FULL_UV;
                        }
                    });
                    if uv != texture.uv {
                        self.mutate(|r| edit::set_uv(r, part_index, uv));
                    }
                }
                ui.separator();
                let skinned = !part.skin.is_empty();
                ui.horizontal(|ui| {
                    ui.label(if skinned {
                        "Muodonmuutos: kärkikohtaiset painot"
                    } else {
                        "Muodonmuutos: jäykkä osa ilman painoja"
                    });
                    if skinned {
                        if ui.button("Normalisoi kaikki").clicked() {
                            self.mutate(|r| edit::normalize_all(r, part_index));
                        }
                        if ui.button("Poista muodonmuutospainot").clicked() {
                            self.mutate(|r| edit::clear_skin(r, part_index));
                        }
                    } else if ui.button("Lisää muodonmuutospainot").clicked() {
                        self.mutate(|r| edit::ensure_skin(r, part_index));
                    }
                });
                if skinned && !part.points.is_empty() {
                    self.vertex = self.vertex.min(part.points.len() - 1);
                    let vertex = self.vertex;
                    ui.horizontal(|ui| {
                        ui.label("Kärki");
                        ui.add(
                            egui::DragValue::new(&mut self.vertex)
                                .range(0..=part.points.len() - 1),
                        );
                        let p = part.points[vertex];
                        ui.weak(format!("kohdassa ({}, {})", p[0], p[1]));
                    });
                    let current = part.skin.get(vertex).cloned().unwrap_or_default();
                    let mut edited = current.clone();
                    let joints: Vec<&str> = rig.joints.iter().map(|j| j.id.as_str()).collect();
                    for (slot, w) in edited.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt(("rig_infl", slot))
                                .selected_text(&w.joint)
                                .show_ui(ui, |ui| {
                                    for j in &joints {
                                        ui.selectable_value(&mut w.joint, (*j).to_string(), *j);
                                    }
                                });
                            ui.add(
                                egui::DragValue::new(&mut w.weight)
                                    .speed(0.01)
                                    .range(0.0..=1.0),
                            );
                        });
                    }
                    ui.horizontal(|ui| {
                        if edited.len() == 1 && ui.button("Lisää toinen nivelvaikutus").clicked() {
                            let other = joints
                                .iter()
                                .find(|j| **j != edited[0].joint)
                                .unwrap_or(&joints[0]);
                            edited.push(VertexWeight {
                                joint: (*other).into(),
                                weight: 0.0,
                            });
                        }
                        if edited.len() == 2 && ui.button("Poista toinen nivelvaikutus").clicked() {
                            edited.truncate(1);
                            edited[0].weight = 1.0;
                        }
                        if ui.button("Normalisoi kärki").clicked() {
                            self.mutate(|r| edit::normalize_vertex(r, part_index, vertex));
                        }
                    });
                    if edited != current {
                        self.mutate(|r| edit::set_influences(r, part_index, vertex, edited));
                    }
                }
                if !self.notice.is_empty() {
                    ui.colored_label(egui::Color32::LIGHT_RED, &self.notice);
                }
                let latest = serde_yaml::from_str::<RigDefinition>(&self.yaml).unwrap_or(rig);
                let issues = edit::issues(&latest, images);
                if issues.is_empty() {
                    ui.small("Tekstuurissa tai muodonmuutospainoissa ei löytynyt ongelmia. Käytä tarkistaa koko projektin uudelleen.");
                }
                for issue in issues {
                    ui.colored_label(egui::Color32::LIGHT_RED, issue);
                }
            });
    }
    /// 1=Apply, 2=restore original humanoid; parent provides context guard/history.
    pub fn ui(&mut self, ui: &mut egui::Ui, images: &[String]) -> u8 {
        ui.heading(format!("{} · mukautettu hahmorakenne", self.actor));
        ui.small("Muokkaa version 1 nivelhierarkiaa ja nivelten värillisiä monikulmioita. Ylänivelen tulee olla ennen alaniveliä. Osien järjestys määrää piirtojärjestyksen. Tämä korvaa alkuperäisen ihmishahmon kuvituksen. Liikekatkelmat muokataan erikseen.");
        ui.add(
            egui::TextEdit::multiline(&mut self.yaml)
                .code_editor()
                .desired_rows(20)
                .desired_width(f32::INFINITY),
        );
        self.texture_skin_ui(ui, images);
        let mut action = 0;
        ui.horizontal(|ui| {
            if ui.button("Käytä hahmorakenne").clicked() {
                action = 1;
            }
            if ui.button("Palauta luonnos").clicked() {
                self.yaml.clone_from(&self.original);
            }
            if ui.button("Käytä alkuperäistä ihmishahmoa").clicked() {
                action = 2;
            }
        });
        action
    }
}
const TEMPLATE: &str = "version: 1\nid: my_character\njoints:\n  - id: root\n    parent: null\n    x: 0\n    y: -40\n  - id: head\n    parent: root\n    x: 0\n    y: -40\nparts:\n  - id: body\n    joint: root\n    points: [[-20, -20], [20, -20], [20, 40], [-20, 40]]\n    color: [70, 160, 220, 255]\n  - id: head_shape\n    joint: head\n    points: [[-18, -18], [18, -18], [18, 18], [-18, 18]]\n    color: [245, 200, 160, 255]\n";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn template_applies_roundtrips_and_invalid_edit_is_atomic() {
        let source = include_str!("../../../examples/hello-emma.yaml");
        let script = motion_script::parse(source).unwrap();
        let actor = script.characters.keys().next().unwrap();
        let mut draft = Draft::for_actor(&script, actor);
        let updated = draft.apply(source).unwrap();
        let parsed = motion_script::parse(&updated).unwrap();
        assert_eq!(
            parsed.characters[actor].rig.as_ref().unwrap().parts.len(),
            2
        );
        draft.yaml = draft.yaml.replace("version: 1", "version: 99");
        assert!(draft.apply(&updated).is_err());
        assert!(motion_script::parse(&draft.remove(&updated).unwrap())
            .unwrap()
            .characters[actor]
            .rig
            .is_none());
        assert!(script.characters[actor].rig.is_none());
    }

    fn test_script() -> Script {
        motion_script::parse(include_str!("../../../examples/hello-emma.yaml")).unwrap()
    }
    fn template_rig() -> RigDefinition {
        serde_yaml::from_str(TEMPLATE).unwrap()
    }
    fn images() -> Vec<String> {
        vec!["skin".into()]
    }
    #[test]
    fn legacy_rig_without_texture_or_skin_roundtrips_byte_compatibly() {
        let rig = template_rig();
        let yaml = serde_yaml::to_string(&rig).unwrap();
        assert!(!yaml.contains("texture") && !yaml.contains("skin"));
        let mut draft = Draft::for_actor(&test_script(), "NOPE");
        draft.yaml.clone_from(&yaml);
        // A no-op mutation rewrites the identical text.
        draft.mutate(|_| Ok(()));
        assert_eq!(draft.yaml, yaml);
        assert!(edit::issues(&rig, &[]).is_empty());
    }
    #[test]
    fn texture_and_uv_transformations() {
        let mut rig = template_rig();
        assert!(edit::set_uv(&mut rig, 0, [0.0, 0.0, 0.5, 0.5]).is_err());
        edit::set_texture(&mut rig, 0, Some("skin")).unwrap();
        edit::set_uv(&mut rig, 0, [0.25, 0.0, 0.75, 0.5]).unwrap();
        // Rebinding keeps the rectangle; unbinding removes the field entirely.
        edit::set_texture(&mut rig, 0, Some("other")).unwrap();
        assert_eq!(
            rig.parts[0].texture.as_ref().unwrap().uv,
            [0.25, 0.0, 0.75, 0.5]
        );
        edit::set_texture(&mut rig, 0, None).unwrap();
        assert!(!serde_yaml::to_string(&rig).unwrap().contains("texture"));
        assert!(edit::set_texture(&mut rig, 9, None).is_err());
        // Out-of-range UVs and unknown assets surface as visible issues.
        edit::set_texture(&mut rig, 0, Some("skin")).unwrap();
        assert!(edit::issues(&rig, &images()).is_empty());
        assert!(edit::issues(&rig, &[])
            .iter()
            .any(|i| i.contains("ei ole projektin kuvaresurssi")));
        edit::set_uv(&mut rig, 0, [0.0, 0.0, 1.5, 1.0]).unwrap();
        assert!(edit::issues(&rig, &images())
            .iter()
            .any(|i| i.starts_with("Hahmorakenteen tarkistus")));
    }
    #[test]
    fn skin_influences_normalize_and_validate() {
        let mut rig = template_rig();
        edit::ensure_skin(&mut rig, 1).unwrap();
        assert_eq!(rig.parts[1].skin.len(), 4);
        assert!(edit::issues(&rig, &[]).is_empty());
        let two = |a: f64, b: f64| {
            vec![
                VertexWeight {
                    joint: "root".into(),
                    weight: a,
                },
                VertexWeight {
                    joint: "head".into(),
                    weight: b,
                },
            ]
        };
        edit::set_influences(&mut rig, 1, 0, two(0.6, 0.6)).unwrap();
        let issues = edit::issues(&rig, &[]);
        assert!(issues.iter().any(|i| i.contains("summa on 1.200000")));
        assert!(issues
            .iter()
            .any(|i| i.starts_with("Hahmorakenteen tarkistus")));
        edit::normalize_vertex(&mut rig, 1, 0).unwrap();
        assert!(edit::issues(&rig, &[]).is_empty());
        assert_eq!(rig.parts[1].skin[0][0].weight, 0.5);
        // Unnormalizable data is rejected without modification.
        edit::set_influences(&mut rig, 1, 1, two(0.0, 0.0)).unwrap();
        let snapshot = rig.clone();
        assert!(edit::normalize_all(&mut rig, 1).is_err());
        assert_eq!(rig, snapshot);
        assert!(edit::set_influences(&mut rig, 1, 1, vec![]).is_err());
        assert!(edit::set_influences(&mut rig, 1, 99, two(1.0, 0.0)).is_err());
        // Unknown joints and duplicate joints are caught by the real validation.
        edit::set_influences(
            &mut rig,
            1,
            1,
            vec![VertexWeight {
                joint: "nope".into(),
                weight: 1.0,
            }],
        )
        .unwrap();
        assert!(edit::issues(&rig, &[])
            .iter()
            .any(|i| i.starts_with("Hahmorakenteen tarkistus")));
        edit::clear_skin(&mut rig, 1).unwrap();
        assert!(!serde_yaml::to_string(&rig).unwrap().contains("skin"));
    }
    #[test]
    fn mutate_reserialises_draft_and_keeps_text_on_failure() {
        let script = test_script();
        let mut draft = Draft::for_actor(&script, "NOPE");
        draft.mutate(|r| edit::set_texture(r, 0, Some("skin")));
        assert!(draft.yaml.contains("asset: skin"));
        let before = draft.yaml.clone();
        draft.mutate(|r| edit::set_uv(r, 7, FULL));
        assert_eq!(draft.yaml, before);
        assert!(draft.notice.contains("ei ole") || !draft.notice.is_empty());
        draft.yaml = "not: [valid".into();
        draft.mutate(|_| Ok(()));
        assert_eq!(draft.yaml, "not: [valid");
        assert!(!draft.notice.is_empty());
    }
    const FULL: [f64; 4] = edit::FULL_UV;
    fn run_ui(draft: &mut Draft, images: &[String]) -> u8 {
        let ctx = egui::Context::default();
        let mut action = 0;
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| action = draft.ui(ui, images));
        });
        action
    }
    #[test]
    fn headless_ui_renders_with_and_without_images_and_malformed_yaml() {
        let script = test_script();
        let mut draft = Draft::for_actor(&script, "NOPE");
        assert_eq!(run_ui(&mut draft, &[]), 0);
        assert_eq!(run_ui(&mut draft, &images()), 0);
        draft.mutate(|r| {
            edit::set_texture(r, 0, Some("gone"))?;
            edit::ensure_skin(r, 0)
        });
        assert_eq!(run_ui(&mut draft, &images()), 0);
        draft.yaml = "version: [".into();
        assert_eq!(run_ui(&mut draft, &images()), 0);
    }
    #[test]
    fn invalid_texture_or_skin_never_replaces_the_project() {
        let source = include_str!("../../../examples/hello-emma.yaml");
        let script = motion_script::parse(source).unwrap();
        let actor = script.characters.keys().next().unwrap().clone();
        let mut draft = Draft::for_actor(&script, &actor);
        let good = draft.apply(source).unwrap();
        for bad in [
            |r: &mut RigDefinition| {
                edit::set_texture(r, 0, Some("skin"))?;
                edit::set_uv(r, 0, [0.0, 0.0, 1.5, 1.0])
            },
            |r: &mut RigDefinition| {
                edit::set_texture(r, 0, Some("skin"))?;
                edit::set_uv(r, 0, [0.5, 0.0, 0.5, 1.0])
            },
            |r: &mut RigDefinition| {
                edit::set_influences(
                    r,
                    0,
                    0,
                    vec![VertexWeight {
                        joint: "root".into(),
                        weight: 0.4,
                    }],
                )
            },
        ] {
            let mut d = Draft::for_actor(&script, &actor);
            d.mutate(bad);
            let result = d.apply(&good);
            assert!(result.is_err(), "{:?}", d.yaml);
        }
        // A valid skin+uv edit applies (texture assets are checked at project level).
        draft.mutate(|r| edit::ensure_skin(r, 0));
        assert!(draft.apply(&good).is_ok());
    }
}
