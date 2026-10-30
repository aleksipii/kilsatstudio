//! Detached live clip instance controls; all mutations use validated source actions.
use eframe::egui;
use motion_script::{ActorEasing, ClipBlendMode, ClipInstance, InstanceWeightKey, Script};

#[derive(Debug, PartialEq)]
pub enum Action {
    Upsert { instance: ClipInstance },
    Remove { id: String },
    Reorder { id: String, new_index: usize },
}

impl Action {
    pub fn apply(self, source: &str, scene_index: usize) -> Result<String, motion_script::Error> {
        match self {
            Self::Upsert { instance } => {
                motion_script::upsert_clip_instance(source, scene_index, instance)
            }
            Self::Remove { id } => motion_script::remove_clip_instance(source, scene_index, &id),
            Self::Reorder { id, new_index } => {
                motion_script::reorder_clip_instance(source, scene_index, &id, new_index)
            }
        }
    }
}

#[derive(Default)]
pub struct State {
    draft: Option<ClipInstance>,
    editing: bool,
    confirm_delete: bool,
    context: Option<(usize, String)>,
}

impl State {
    pub fn ui(
        &mut self,
        ui: &mut egui::Ui,
        script: &Script,
        scene_index: usize,
        actor: &str,
    ) -> Option<Action> {
        let context = (scene_index, actor.to_owned());
        if self.context.as_ref() != Some(&context) {
            self.draft = None;
            self.editing = false;
            self.confirm_delete = false;
            self.context = Some(context);
        }
        let scene = script.scenes.get(scene_index)?;
        ui.small("Suorat viittaukset yhdistetään tallennetussa rivijärjestyksessä. Ryhmättömien viittausten painoja ei normalisoida. Lisäävän ja korvaavan tilan järjestys vaikuttaa tulokseen. Aikaväli sisältää alun mutta ei loppuhetkeä. Häivytykset pehmentävät rajoja.");
        ui.small("Kirjaston katkelman muokkaus vaikuttaa jokaiseen suoraan viittaukseen. Käyttäminen ja poistaminen muuttavat käsikirjoitusta, ja ne voi kumota. YAML-muotoilu yhtenäistetään ja kommentit poistetaan.");
        let mut action = None;
        for (index, instance) in scene.composition.clip_instances.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} · {} · {}",
                    instance.id, instance.actor, instance.clip
                ));
                if instance.actor == actor {
                    if ui.button("Muokkaa").clicked() {
                        self.draft = Some(instance.clone());
                        self.editing = true;
                        self.confirm_delete = false;
                    }
                    if ui
                        .add_enabled(index > 0, egui::Button::new("Ylös"))
                        .clicked()
                    {
                        action = Some(Action::Reorder {
                            id: instance.id.clone(),
                            new_index: index - 1,
                        });
                    }
                    if ui
                        .add_enabled(
                            index + 1 < scene.composition.clip_instances.len(),
                            egui::Button::new("Alas"),
                        )
                        .clicked()
                    {
                        action = Some(Action::Reorder {
                            id: instance.id.clone(),
                            new_index: index + 1,
                        });
                    }
                }
            });
        }
        let actor_count = scene
            .composition
            .clip_instances
            .iter()
            .filter(|instance| instance.actor == actor)
            .count();
        let total: usize = script
            .scenes
            .iter()
            .map(|scene| scene.composition.clip_instances.len())
            .sum();
        let can_add = script.characters.contains_key(actor)
            && !script.animation_clips.is_empty()
            && scene.composition.clip_instances.len() < 128
            && actor_count < 32
            && total < 1024;
        if ui
            .add_enabled(can_add, egui::Button::new("Uusi suora katkelmaviittaus"))
            .clicked()
        {
            if let Some((name, clip)) = script.animation_clips.iter().next() {
                self.draft = Some(ClipInstance {
                    id: String::new(),
                    clip: name.clone(),
                    actor: actor.to_owned(),
                    at: 0.0,
                    duration: clip.duration,
                    weight: 1.0,
                    blend_mode: ClipBlendMode::Override,
                    normalized_group: None,
                    fade_in: 0.0,
                    fade_out: 0.0,
                    weight_keys: Vec::new(),
                    looping: false,
                    joint_mask: Vec::new(),
                });
                self.editing = false;
                self.confirm_delete = false;
            }
        }
        let Some(draft) = self.draft.as_mut() else {
            return action;
        };
        if self.editing {
            ui.label(format!("Viittauksen tunnus: {}", draft.id));
        } else if ui.text_edit_singleline(&mut draft.id).changed() {
            self.confirm_delete = false;
        }
        ui.small(
            "Tunnus: 1–64 ASCII-kirjainta, numeroa, alaviivaa tai yhdysmerkkiä. Tunnusta ei voi muuttaa viittausta muokatessa.",
        );
        let previous = draft.clip.clone();
        egui::ComboBox::from_id_salt("live_clip_reference")
            .selected_text(&draft.clip)
            .show_ui(ui, |ui| {
                for name in script.animation_clips.keys() {
                    ui.selectable_value(&mut draft.clip, name.clone(), name);
                }
            });
        if previous != draft.clip {
            self.confirm_delete = false;
        }
        egui::ComboBox::from_id_salt("live_clip_blend_mode")
            .selected_text(blend_mode_label(draft.blend_mode))
            .show_ui(ui, |ui| {
                for mode in [ClipBlendMode::Override, ClipBlendMode::Additive] {
                    if ui
                        .selectable_value(&mut draft.blend_mode, mode, blend_mode_label(mode))
                        .changed()
                    {
                        self.confirm_delete = false;
                        if mode == ClipBlendMode::Additive {
                            draft.normalized_group = None;
                        }
                    }
                }
            });
        let mut grouped = draft.normalized_group.is_some();
        if ui
            .checkbox(&mut grouped, "Normalisoi korvaavien liikkeiden ryhmä")
            .changed()
        {
            draft.normalized_group = grouped.then(|| "group".into());
            if grouped {
                draft.blend_mode = ClipBlendMode::Override;
            }
            self.confirm_delete = false;
        }
        if let Some(group) = &mut draft.normalized_group {
            ui.horizontal(|ui| {
                ui.label("Ryhmän tunnus");
                if ui.text_edit_singleline(group).changed() {
                    self.confirm_delete = false;
                }
            });
            ui.small("Käytä samaa tunnusta yhden hahmon peräkkäisillä korvaavilla riveillä. Nivelen aktiivisten liikkeiden positiiviset painot normalisoidaan, kun summa ylittää yhden. Alle yhden jäävän osuuden täyttää aiempi asento. Ryhmän paikka säilyttää ympäröivien liikkeiden järjestyksen.");
        }
        ui.small(match draft.blend_mode {
            ClipBlendMode::Override => "Korvaava tila siirtää yhteisten nivelten aiempia siirtymiä kohti tätä katkelmaa. Kierto kulkee lyhintä kaarta.",
            ClipBlendMode::Additive => "Lisäävä tila lisää aiempiin siirtymiin painotetun siirtymän ja kierron muutoksen radiaaneina. Kierron suunta ja kierrosten määrä säilyvät. Mittakaava kerrotaan arvolla 1 + vaikutuspaino × (katkelman mittakaava − 1); mittakaava 1 on neutraali. Yhdistettyjen arvojen tulee pysyä sallituissa rajoissa.",
        });
        if ui
            .checkbox(
                &mut draft.looping,
                "Toista katkelma alkuperäisellä nopeudella",
            )
            .changed()
        {
            self.confirm_delete = false;
        }
        ui.small("Toisto käyttää kirjastokatkelman alkuperäistä kestoa. Häivytykset ja painojen avainruudut kattavat koko viittauksen. Ilman toistoa katkelma venytetään kerran viittauksen kestoon.");
        let mut masked = !draft.joint_mask.is_empty();
        if ui
            .checkbox(&mut masked, "Valitse yksittäiset nivelet")
            .changed()
        {
            draft.joint_mask = if masked {
                motion_script::HUMANOID_BONES
                    .iter()
                    .map(|name| (*name).to_owned())
                    .collect()
            } else {
                Vec::new()
            };
            self.confirm_delete = false;
        }
        if masked {
            for name in motion_script::HUMANOID_BONES {
                let mut selected = draft.joint_mask.iter().any(|joint| joint == name);
                if ui.checkbox(&mut selected, name).changed() {
                    if selected {
                        draft.joint_mask.push(name.to_owned());
                    } else if draft.joint_mask.len() > 1 {
                        draft.joint_mask.retain(|joint| joint != name);
                    }
                    self.confirm_delete = false;
                }
            }
            ui.small("Valitse ainakin yksi nivel. Valinta vaikuttaa vain näiden nivelten siirtymiin, eikä laajene alaniveliin. Poista valintarajaus käyttääksesi katkelman kaikkia niveliä.");
        }
        for (label, value) in [
            ("Alku (sekuntia)", &mut draft.at),
            ("Kesto (sekuntia)", &mut draft.duration),
            (weight_label(draft.blend_mode), &mut draft.weight),
            ("Alkuhäivytys (sekuntia)", &mut draft.fade_in),
            ("Loppuhäivytys (sekuntia)", &mut draft.fade_out),
        ] {
            ui.horizontal(|ui| {
                ui.label(label);
                if number_field(ui, value) {
                    self.confirm_delete = false;
                }
            });
        }
        ui.small("Painojen avainruutujen ajat ovat suhteessa tämän viittauksen alkuun. Avainruudun arvo kerrotaan valitun tilan painokertoimella ja lineaarisilla häivytyksillä. Päätearvot säilyvät aikavälin ulkopuolella, ja siirtymätapa vaikuttaa avainruudun jälkeen. Keston muokkaus ei skaalaa avainruutujen tai häivytysten aikoja.");
        let mut remove = None;
        for (index, key) in draft.weight_keys.iter_mut().enumerate() {
            ui.push_id(index, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Aika");
                    let time_changed = number_field(ui, &mut key.time);
                    ui.label("Paino (0–1)");
                    let weight_changed = number_field(ui, &mut key.weight);
                    if time_changed || weight_changed {
                        self.confirm_delete = false;
                    }
                    egui::ComboBox::from_id_salt("instance_weight_easing")
                        .selected_text(easing_label(key.easing))
                        .show_ui(ui, |ui| {
                            for easing in [
                                ActorEasing::Linear,
                                ActorEasing::Smoothstep,
                                ActorEasing::Hold,
                            ] {
                                if ui
                                    .selectable_value(&mut key.easing, easing, easing_label(easing))
                                    .changed()
                                {
                                    self.confirm_delete = false;
                                }
                            }
                        });
                    if ui.button("Poista avainruutu").clicked() {
                        remove = Some(index);
                    }
                });
            });
        }
        if let Some(index) = remove {
            draft.weight_keys.remove(index);
            self.confirm_delete = false;
        }
        let next_time = next_key_time(draft);
        if ui
            .add_enabled(
                next_time.is_some()
                    && envelope_valid(draft)
                    && mask_valid(draft)
                    && envelope_budget(script, scene_index, draft, self.editing, 1),
                egui::Button::new("Lisää painon avainruutu"),
            )
            .clicked()
        {
            if let Some(time) = next_time {
                draft.weight_keys.push(InstanceWeightKey {
                    time,
                    weight: 1.0,
                    easing: ActorEasing::Linear,
                });
                self.confirm_delete = false;
            }
        }
        if ui
            .button("Palauta ajoitus, paino ja häivytykset; tyhjennä painon avainruudut")
            .clicked()
        {
            reset_envelope(draft, script);
            self.confirm_delete = false;
        }
        let valid_id = !draft.id.is_empty()
            && draft.id.len() <= 64
            && draft
                .id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-');
        let valid = valid_id
            && normalized_group_valid(&scene.composition.clip_instances, draft, self.editing)
            && script.animation_clips.contains_key(&draft.clip)
            && draft.at.is_finite()
            && draft.at >= 0.0
            && draft.duration.is_finite()
            && draft.duration > 0.0
            && (draft.at + draft.duration).is_finite()
            && draft.at + draft.duration > draft.at
            && draft.at + draft.duration <= scene.duration
            && draft.weight.is_finite()
            && (0.0..=1.0).contains(&draft.weight)
            && envelope_valid(draft)
            && envelope_budget(script, scene_index, draft, self.editing, 0)
            && (self.editing
                || (can_add
                    && !scene
                        .composition
                        .clip_instances
                        .iter()
                        .any(|instance| instance.id == draft.id)));
        if !valid {
            ui.colored_label(egui::Color32::LIGHT_RED, "Valitse olemassa oleva katkelma ja yksilöllinen tunnus. Ajoituksen tulee olla äärellinen ja kohtauksen sisällä. Painojen tulee olla väliltä 0–1, eivätkä häivytykset saa mennä päällekkäin. Avainruutujen aikojen tulee kasvaa viittauksen sisällä. Avainruutuja saa olla enintään 256 viittausta kohden, 8192 kohtausta kohden ja 65536 projektia kohden. Normalisoitu ryhmä koostuu yhden hahmon peräkkäisistä korvaavista riveistä ja tarvitsee kelvollisen ryhmätunnuksen.");
        }
        if ui
            .add_enabled(valid, egui::Button::new("Käytä suora katkelmaviittaus"))
            .clicked()
        {
            action = Some(Action::Upsert {
                instance: draft.clone(),
            });
            self.confirm_delete = false;
        }
        if self.editing {
            ui.checkbox(
                &mut self.confirm_delete,
                "Vahvista tämän suoran viittauksen poistaminen",
            );
            if ui
                .add_enabled(
                    self.confirm_delete,
                    egui::Button::new("Poista suora katkelmaviittaus"),
                )
                .clicked()
            {
                action = Some(Action::Remove {
                    id: draft.id.clone(),
                });
                self.confirm_delete = false;
            }
        }
        action
    }
}

fn reset_envelope(draft: &mut ClipInstance, script: &Script) {
    draft.at = 0.0;
    draft.duration = script
        .animation_clips
        .get(&draft.clip)
        .map_or(0.0, |clip| clip.duration);
    draft.weight = 1.0;
    draft.fade_in = 0.0;
    draft.fade_out = 0.0;
    draft.weight_keys.clear();
}

fn normalized_group_valid(rows: &[ClipInstance], draft: &ClipInstance, editing: bool) -> bool {
    let mut candidates = rows
        .iter()
        .map(|row| {
            if editing && row.id == draft.id {
                draft
            } else {
                row
            }
        })
        .collect::<Vec<_>>();
    if !editing {
        candidates.push(draft);
    }
    let mut seen = std::collections::BTreeSet::new();
    let mut previous: Option<(&str, &str)> = None;
    for row in candidates {
        if let Some(group) = row.normalized_group.as_deref() {
            if group.is_empty()
                || group.len() > 64
                || !group
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
                || row.blend_mode != ClipBlendMode::Override
            {
                return false;
            }
            if previous != Some((group, row.actor.as_str())) && !seen.insert(group) {
                return false;
            }
            previous = Some((group, row.actor.as_str()));
        } else {
            previous = None;
        }
    }
    true
}

fn mask_valid(draft: &ClipInstance) -> bool {
    let mut selected = std::collections::BTreeSet::new();
    draft.joint_mask.len() <= motion_script::HUMANOID_BONES.len()
        && draft.joint_mask.iter().all(|joint| {
            motion_script::HUMANOID_BONES.contains(&joint.as_str()) && selected.insert(joint)
        })
}

fn envelope_valid(draft: &ClipInstance) -> bool {
    draft.duration.is_finite()
        && draft.duration > 0.0
        && draft.fade_in.is_finite()
        && draft.fade_in >= 0.0
        && draft.fade_out.is_finite()
        && draft.fade_out >= 0.0
        && draft.fade_in + draft.fade_out <= draft.duration
        && draft.weight_keys.len() <= 256
        && draft.weight_keys.iter().all(|key| {
            key.time.is_finite()
                && key.time >= 0.0
                && key.time <= draft.duration
                && key.weight.is_finite()
                && (0.0..=1.0).contains(&key.weight)
        })
        && draft
            .weight_keys
            .windows(2)
            .all(|pair| pair[0].time < pair[1].time)
}

fn next_key_time(draft: &ClipInstance) -> Option<f64> {
    if !draft.duration.is_finite() || draft.duration <= 0.0 {
        return None;
    }
    match draft.weight_keys.last() {
        None => Some(0.0),
        Some(last) if last.time.is_finite() && last.time < draft.duration => {
            let time = (last.time + 1.0).min(draft.duration);
            (time > last.time).then_some(time)
        }
        _ => None,
    }
}

fn envelope_budget(
    script: &Script,
    scene_index: usize,
    draft: &ClipInstance,
    editing: bool,
    extra: usize,
) -> bool {
    let mut project_count = draft.weight_keys.len() + extra;
    let mut scene_count = project_count;
    for (index, scene) in script.scenes.iter().enumerate() {
        for instance in &scene.composition.clip_instances {
            if editing && index == scene_index && instance.id == draft.id {
                continue;
            }
            project_count += instance.weight_keys.len();
            if index == scene_index {
                scene_count += instance.weight_keys.len();
            }
        }
    }
    draft.weight_keys.len() + extra <= 256 && scene_count <= 8192 && project_count <= 65536
}

fn blend_mode_label(mode: ClipBlendMode) -> &'static str {
    match mode {
        ClipBlendMode::Override => "Korvaava",
        ClipBlendMode::Additive => "Lisäävä",
    }
}

fn weight_label(mode: ClipBlendMode) -> &'static str {
    match mode {
        ClipBlendMode::Override => "Korvaavan liikkeen paino (0–1)",
        ClipBlendMode::Additive => "Lisäävän liikkeen painokerroin (0–1)",
    }
}

fn easing_label(easing: ActorEasing) -> &'static str {
    match easing {
        ActorEasing::Linear => "Lineaarinen",
        ActorEasing::Smoothstep => "Pehmeä",
        ActorEasing::Hold => "Pidä arvo",
        ActorEasing::EaseIn => "Hidas alku",
        ActorEasing::EaseOut => "Hidas loppu",
        ActorEasing::EaseInOut => "Hidas alku ja loppu",
        ActorEasing::Back(_) => "Ylilyönti",
    }
}

fn number_field(ui: &mut egui::Ui, value: &mut f64) -> bool {
    if !value.is_finite() {
        ui.colored_label(egui::Color32::LIGHT_RED, value.to_string());
        return false;
    }
    let mut edited = *value;
    if ui
        .add(egui::DragValue::new(&mut edited).speed(0.05))
        .changed()
    {
        *value = edited;
        true
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const SOURCE: &str = "characters: {A: {asset: a}, B: {asset: b}}\nscenes: [{id: shot, duration: 4, events: [], composition: {bone_tracks: {A: {head: [{at: 0, x: 2}, {at: 2, x: 8}]}}}}]\n";

    #[test]
    fn normalized_group_drafts_apply_roundtrip_reset_and_reject_invalid_blocks() {
        let mut first = instance("a");
        first.normalized_group = Some("reach".into());
        let saved = Action::Upsert {
            instance: first.clone(),
        }
        .apply(&captured(), 0)
        .unwrap();
        let mut second = instance("b");
        second.normalized_group = Some("reach".into());
        let script = motion_script::parse(&saved).unwrap();
        assert!(normalized_group_valid(
            &script.scenes[0].composition.clip_instances,
            &second,
            false
        ));
        let saved = Action::Upsert {
            instance: second.clone(),
        }
        .apply(&saved, 0)
        .unwrap();
        let reopened = motion_script::parse(&saved).unwrap();
        assert_eq!(reopened.scenes[0].composition.clip_instances[1], second);
        reset_envelope(&mut second, &reopened);
        assert_eq!(second.normalized_group.as_deref(), Some("reach"));
        second.actor = "A".into();
        assert!(!normalized_group_valid(
            &reopened.scenes[0].composition.clip_instances,
            &second,
            true
        ));
        assert!(Action::Upsert { instance: second }
            .apply(&saved, 0)
            .is_err());
        let mut gap = instance("gap");
        gap.normalized_group = None;
        let saved_gap = Action::Upsert { instance: gap }.apply(&saved, 0).unwrap();
        let mut after = instance("after");
        after.normalized_group = Some("reach".into());
        let parsed = motion_script::parse(&saved_gap).unwrap();
        assert!(!normalized_group_valid(
            &parsed.scenes[0].composition.clip_instances,
            &after,
            false
        ));
        assert!(Action::Upsert { instance: after }
            .apply(&saved_gap, 0)
            .is_err());
    }

    #[test]
    fn additive_draft_applies_in_saved_order_and_invalid_edit_preserves_saved_mode() {
        let source = captured();
        let override_row = instance("base");
        assert_eq!(override_row.blend_mode, ClipBlendMode::Override);
        let saved = Action::Upsert {
            instance: override_row.clone(),
        }
        .apply(&source, 0)
        .unwrap();
        let mut additive = instance("delta");
        additive.blend_mode = ClipBlendMode::Additive;
        let applied = Action::Upsert {
            instance: additive.clone(),
        }
        .apply(&saved, 0)
        .unwrap();
        let reopened = motion_script::parse(&applied).unwrap();
        assert_eq!(
            reopened.scenes[0].composition.clip_instances,
            vec![override_row.clone(), additive.clone()]
        );
        let reordered = Action::Reorder {
            id: "delta".into(),
            new_index: 0,
        }
        .apply(&applied, 0)
        .unwrap();
        assert_eq!(
            motion_script::parse(&reordered).unwrap().scenes[0]
                .composition
                .clip_instances,
            vec![additive.clone(), override_row]
        );
        additive.blend_mode = ClipBlendMode::Override;
        additive.weight = f64::INFINITY;
        assert!(Action::Upsert { instance: additive }
            .apply(&reordered, 0)
            .is_err());
        assert_eq!(
            motion_script::parse(&reordered).unwrap().scenes[0]
                .composition
                .clip_instances[0]
                .blend_mode,
            ClipBlendMode::Additive
        );
        assert!(motion_script::parse(&source).unwrap().scenes[0]
            .composition
            .clip_instances
            .is_empty());
    }

    #[test]
    fn looping_joint_selection_is_detached_persisted_and_validated() {
        let source = captured();
        let mut draft = instance("masked");
        draft.looping = true;
        draft.joint_mask = vec!["head".into()];
        assert!(mask_valid(&draft));
        assert!(motion_script::parse(&source).unwrap().scenes[0]
            .composition
            .clip_instances
            .is_empty());
        let saved = Action::Upsert {
            instance: draft.clone(),
        }
        .apply(&source, 0)
        .unwrap();
        assert_eq!(
            motion_script::parse(&saved).unwrap().scenes[0]
                .composition
                .clip_instances[0],
            draft
        );
        draft.joint_mask.push("head".into());
        assert!(!mask_valid(&draft));
        assert!(Action::Upsert { instance: draft }.apply(&saved, 0).is_err());
    }

    fn captured() -> String {
        motion_script::capture_animation_clip(SOURCE, 0, "A", "Nod", false).unwrap()
    }

    fn instance(id: &str) -> ClipInstance {
        ClipInstance {
            id: id.into(),
            clip: "Nod".into(),
            actor: "B".into(),
            at: 0.0,
            duration: 4.0,
            weight: 0.5,
            blend_mode: ClipBlendMode::Override,
            normalized_group: None,
            fade_in: 0.0,
            fade_out: 0.0,
            weight_keys: Vec::new(),
            looping: false,
            joint_mask: Vec::new(),
        }
    }

    #[test]
    fn explicit_actions_upsert_reorder_remove_and_reject_invalid_source_edits() {
        let source = captured();
        let first = Action::Upsert {
            instance: instance("first"),
        }
        .apply(&source, 0)
        .unwrap();
        let second = Action::Upsert {
            instance: instance("second"),
        }
        .apply(&first, 0)
        .unwrap();
        let reordered = Action::Reorder {
            id: "second".into(),
            new_index: 0,
        }
        .apply(&second, 0)
        .unwrap();
        assert_eq!(
            motion_script::parse(&reordered).unwrap().scenes[0]
                .composition
                .clip_instances[0]
                .id,
            "second"
        );
        let removed = Action::Remove { id: "first".into() }
            .apply(&reordered, 0)
            .unwrap();
        assert_eq!(
            motion_script::parse(&removed).unwrap().scenes[0]
                .composition
                .clip_instances
                .len(),
            1
        );
        let mut bad = instance("bad");
        bad.weight = f64::NAN;
        assert!(Action::Upsert { instance: bad }.apply(&source, 0).is_err());
        assert!(motion_script::parse(&source).unwrap().scenes[0]
            .composition
            .clip_instances
            .is_empty());
    }

    #[test]
    fn painting_preserves_invalid_detached_draft_and_context_clears_removal_confirmation() {
        let script = motion_script::parse(&captured()).unwrap();
        let before = serde_json::to_value(&script).unwrap();
        let mut draft = instance("draft");
        draft.blend_mode = ClipBlendMode::Additive;
        draft.at = f64::NAN;
        draft.weight = f64::INFINITY;
        draft.fade_in = f64::NAN;
        draft.fade_out = f64::INFINITY;
        draft.weight_keys.push(InstanceWeightKey {
            time: f64::NAN,
            weight: f64::INFINITY,
            easing: ActorEasing::Linear,
        });
        let mut state = State {
            draft: Some(draft),
            editing: false,
            confirm_delete: true,
            context: Some((0, "B".into())),
        };
        let ctx = egui::Context::default();
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(state.ui(ui, &script, 0, "B").is_none());
            });
        });
        assert_eq!(
            state.draft.as_ref().unwrap().blend_mode,
            ClipBlendMode::Additive
        );
        assert!(state.draft.as_ref().unwrap().at.is_nan());
        assert!(state.draft.as_ref().unwrap().fade_in.is_nan());
        assert!(state.draft.as_ref().unwrap().weight_keys[0].time.is_nan());
        assert_eq!(
            state.draft.as_ref().unwrap().weight_keys[0].weight,
            f64::INFINITY
        );
        assert_eq!(state.draft.as_ref().unwrap().weight, f64::INFINITY);
        let _ = ctx.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                assert!(state.ui(ui, &script, 0, "A").is_none());
            });
        });
        assert!(state.draft.is_none());
        assert!(!state.confirm_delete);
        assert_eq!(serde_json::to_value(&script).unwrap(), before);
    }

    #[test]
    fn envelopes_persist_and_invalid_duration_changes_do_not_touch_source() {
        let source = captured();
        let mut draft = instance("envelope");
        draft.fade_in = 0.5;
        draft.fade_out = 1.0;
        draft.weight_keys = vec![
            InstanceWeightKey {
                time: 0.0,
                weight: 0.2,
                easing: ActorEasing::Smoothstep,
            },
            InstanceWeightKey {
                time: 4.0,
                weight: 0.8,
                easing: ActorEasing::Hold,
            },
        ];
        assert!(envelope_valid(&draft));
        let applied = Action::Upsert {
            instance: draft.clone(),
        }
        .apply(&source, 0)
        .unwrap();
        let saved = motion_script::parse(&applied).unwrap();
        assert_eq!(saved.scenes[0].composition.clip_instances[0], draft);
        draft.duration = 1.0;
        assert!(!envelope_valid(&draft));
        assert!(Action::Upsert { instance: draft }
            .apply(&applied, 0)
            .is_err());
        assert_eq!(
            motion_script::parse(&applied).unwrap().scenes[0]
                .composition
                .clip_instances[0]
                .duration,
            4.0
        );
    }

    #[test]
    fn key_addition_handles_subsecond_windows_and_explicit_reset_recovers_invalid_values() {
        let script = motion_script::parse(&captured()).unwrap();
        let mut draft = instance("draft");
        draft.blend_mode = ClipBlendMode::Additive;
        draft.looping = true;
        draft.joint_mask = vec!["head".into()];
        draft.duration = 0.25;
        assert_eq!(next_key_time(&draft), Some(0.0));
        draft.weight_keys.push(InstanceWeightKey {
            time: 0.0,
            weight: 1.0,
            easing: ActorEasing::Linear,
        });
        assert_eq!(next_key_time(&draft), Some(0.25));
        draft.weight_keys.push(InstanceWeightKey {
            time: 0.25,
            weight: 1.0,
            easing: ActorEasing::Linear,
        });
        assert_eq!(next_key_time(&draft), None);
        draft.fade_in = f64::NAN;
        draft.fade_out = f64::INFINITY;
        draft.weight = f64::NAN;
        draft.weight_keys[0].time = f64::NAN;
        assert!(!envelope_valid(&draft));
        reset_envelope(&mut draft, &script);
        assert!(envelope_valid(&draft));
        assert_eq!(
            (draft.at, draft.weight, draft.fade_in, draft.fade_out),
            (0.0, 1.0, 0.0, 0.0)
        );
        assert!(draft.weight_keys.is_empty());
        assert_eq!(draft.blend_mode, ClipBlendMode::Additive);
        assert!(draft.looping);
        assert_eq!(draft.joint_mask, vec!["head"]);
    }

    #[test]
    fn envelope_budgets_count_other_actors_and_exclude_only_edited_instance() {
        let mut script = motion_script::parse(&captured()).unwrap();
        let key = InstanceWeightKey {
            time: 0.0,
            weight: 1.0,
            easing: ActorEasing::Linear,
        };
        let mut draft = instance("draft");
        draft.weight_keys = vec![key.clone(); 256];
        script.scenes[0].composition.clip_instances = (0..32)
            .map(|i| {
                let mut row = instance(&format!("row_{i}"));
                row.actor = "A".into();
                row.weight_keys = vec![key.clone(); 256];
                row
            })
            .collect();
        assert!(!envelope_budget(&script, 0, &draft, false, 0));
        draft.id = "row_0".into();
        assert!(envelope_budget(&script, 0, &draft, true, 0));
        assert!(!envelope_budget(&script, 0, &draft, true, 1));
        let scene = script.scenes[0].clone();
        script.scenes = vec![scene; 8];
        assert!(envelope_budget(&script, 0, &draft, true, 0));
        script.scenes.push(script.scenes[0].clone());
        assert!(!envelope_budget(&script, 0, &draft, true, 0));
    }
}
