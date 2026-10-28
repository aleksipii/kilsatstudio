//! Offline Finnish help, shared with the checked-in user guide.
use eframe::egui;

const GUIDE: &str = include_str!("../KAYTTOOHJE.md");

fn sections() -> Vec<(&'static str, &'static str)> {
    GUIDE
        .split("\n## ")
        .skip(1)
        .filter_map(|section| section.split_once('\n'))
        .collect()
}

pub fn show(ctx: &egui::Context, open: &mut bool, topic: &mut usize) {
    egui::Window::new("Käyttöohjeet · Kilsat Studio")
        .id(egui::Id::new("finnish-help"))
        .open(open)
        .default_size(egui::vec2(700.0, 520.0))
        .min_width(280.0)
        .show(ctx, |ui| {
            let topics = sections();
            *topic = (*topic).min(topics.len().saturating_sub(1));
            ui.weak("Ohje toimii ilman verkkoyhteyttä. Valitse aihe:");
            egui::ComboBox::from_id_salt("help-topic")
                .selected_text(topics[*topic].0)
                .width(ui.available_width().min(440.0))
                .show_ui(ui, |ui| {
                    for (index, (title, _)) in topics.iter().enumerate() {
                        ui.selectable_value(topic, index, *title);
                    }
                });
            ui.separator();
            egui::ScrollArea::vertical()
                .id_salt(("help-body", *topic))
                .max_height(580.0)
                .show(ui, |ui| {
                    ui.heading(topics[*topic].0);
                    for paragraph in topics[*topic].1.trim().split("\n\n") {
                        ui.label(paragraph.replace('\n', " "));
                        ui.add_space(5.0);
                    }
                });
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_help_topic_draws_offline_from_the_same_user_guide() {
        let topics = sections();
        assert_eq!(topics.len(), 12);
        for (index, (title, body)) in topics.iter().enumerate() {
            assert!(!body.trim().is_empty());
            let ctx = egui::Context::default();
            let mut open = true;
            let mut selected = index;
            // A second frame resolves the new window's initial sizing pass.
            let _ = ctx.run(Default::default(), |ctx| {
                show(ctx, &mut open, &mut selected)
            });
            {
                let output = ctx.run(Default::default(), |ctx| {
                    show(ctx, &mut open, &mut selected)
                });
                assert!(
                    output.shapes.iter().any(|shape| match &shape.shape {
                        egui::epaint::Shape::Text(text) => text.galley.text().contains(title),
                        _ => false,
                    }),
                    "missing guide topic: {title}"
                );
            }
            assert!(open);
            assert_eq!(selected, index);
        }
    }
}
