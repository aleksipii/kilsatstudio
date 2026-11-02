//! Read-only list of the animation-principle staging hints for one scene.
use eframe::egui;
use motion_script::{check_staging, Script, StagingCheck, StagingNote};

/// Finnish one-line summary of a hint.
pub fn describe(note: &StagingNote) -> String {
    let actions = note
        .actions
        .iter()
        .map(|a| {
            let kind = if a.kind == "move" { "liike" } else { "ele" };
            format!("{} ({kind})", a.actor)
        })
        .collect::<Vec<_>>()
        .join(", ");
    match note.check {
        StagingCheck::SimultaneousActions => format!(
            "Ruudut {}–{}: {actions} alkavat ja loppuvat samassa ruudussa. Porrasta 2–4 ruudulla, esimerkiksi stagger-operaatiolla ja viiveellä 3f.",
            note.start_frame, note.end_frame
        ),
        StagingCheck::AnticipationShortened => format!(
            "Ruutu {}: {actions}: anticipation_frames ei mahdu 30 prosenttiin eleestä, joten ennakointia lyhennetään. Pidennä elettä tai pienennä ruutumäärää.",
            note.start_frame
        ),
    }
}

/// Hints of `scene_index`, from the current script; computed only while open.
pub fn ui(ui: &mut egui::Ui, script: Option<&Script>, scene_index: usize) {
    egui::CollapsingHeader::new("Animaatioperiaatteiden vihjeet")
        .show(ui, |ui| body(ui, script, scene_index));
}

fn body(ui: &mut egui::Ui, script: Option<&Script>, scene_index: usize) {
    let Some(script) = script else {
        ui.weak("Avaa käsikirjoitus nähdäksesi vihjeet.");
        return;
    };
    let notes: Vec<_> = check_staging(script)
        .into_iter()
        .filter(|note| note.scene_index == scene_index)
        .collect();
    if notes.is_empty() {
        ui.weak("Ei vihjeitä: toiminnot on porrastettu, ja ennakoinnit mahtuvat eleisiin.");
    }
    for note in &notes {
        ui.label(describe(note));
    }
    ui.small("Vihjeet eivät muuta käsikirjoitusta eivätkä estä renderöintiä.");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Texts painted by one headless frame of `body`.
    fn painted(script: Option<&Script>, scene_index: usize) -> String {
        let context = egui::Context::default();
        let output = context.run(egui::RawInput::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| body(ui, script, scene_index));
        });
        output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Text(text) => Some(text.galley.job.text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn headless_panel_paints_the_hints_of_the_selected_scene() {
        let script = motion_script::parse("project: {title: T, fps: 24, width: 640, height: 360}\ncharacters:\n  A: {asset: a}\n  B: {asset: b}\nscenes:\n  - id: s\n    duration: 4\n    events:\n      - {type: move, actor: A, at: 1, x: 10, y: 20, duration: 1}\n      - {type: move, actor: B, at: 1, x: 30, y: 20, duration: 1}\n  - id: t\n    duration: 4\n    events: []\n").unwrap();
        let first = painted(Some(&script), 0);
        assert!(
            first.contains("Ruudut 24–48: A (liike), B (liike)"),
            "{first}"
        );
        let second = painted(Some(&script), 1);
        assert!(second.contains("Ei vihjeitä"), "{second}");
        assert!(painted(None, 0).contains("Avaa käsikirjoitus"));
    }

    #[test]
    fn simultaneous_actions_are_described_in_finnish_with_frames() {
        let script = motion_script::parse("project: {title: T, fps: 24, width: 640, height: 360}\ncharacters:\n  A: {asset: a}\n  B: {asset: b}\nscenes:\n  - id: s\n    duration: 4\n    events:\n      - {type: move, actor: A, at: 1, x: 10, y: 20, duration: 1}\n      - {type: move, actor: B, at: 1, x: 30, y: 20, duration: 1}\n").unwrap();
        let notes = check_staging(&script);
        assert_eq!(notes.len(), 1);
        let text = describe(&notes[0]);
        assert!(
            text.starts_with("Ruudut 24–48: A (liike), B (liike)"),
            "{text}"
        );
        assert!(text.contains("3f"), "{text}");
    }
}
