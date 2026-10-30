//! Prop cards (PRO P06): a deterministic SVG of one catalog item in every
//! state, held by a stick figure at the figure's own scale.
//!
//! The card has one cell per state, left to right. Each cell shows the figure
//! with the item in its hand (same scale in every cell, with a height ruler in
//! rig units) and a close-up of the item alone, labelled with its zoom factor.
//! The output depends only on its inputs, and numbers are written with fixed
//! precision, so the same item and figure always give byte-identical SVG.
use crate::{
    attach_hand_prop, held_part_id, EvaluatedPart, GripPlacement, HandPropItem, HandSide,
    OutlineMode, PropError, RigDefinition, Vec2,
};
use std::fmt::Write as _;

/// Width of one state cell, in SVG pixels.
pub const PROP_CARD_CELL_WIDTH: u32 = 360;
/// Height of one state cell, in SVG pixels.
pub const PROP_CARD_CELL_HEIGHT: u32 = 440;
/// Height of the title band above the cells.
pub const PROP_CARD_HEADER_HEIGHT: u32 = 48;
const LABEL: f64 = 32.0;
const PAD: f64 = 14.0;

/// A rendered prop card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropCard {
    pub svg: String,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Copy)]
struct Bounds {
    min: Vec2,
    max: Vec2,
}
fn bounds<'a>(parts: impl IntoIterator<Item = &'a EvaluatedPart>) -> Option<Bounds> {
    let mut points = parts.into_iter().flat_map(|p| &p.points);
    let first = *points.next()?;
    Some(points.fold(
        Bounds {
            min: first,
            max: first,
        },
        |b, p| Bounds {
            min: Vec2::new(b.min.x.min(p.x), b.min.y.min(p.y)),
            max: Vec2::new(b.max.x.max(p.x), b.max.y.max(p.y)),
        },
    ))
}
/// Uniform scale and offset placing `b` centred in the panel `[x, y, w, h]`.
fn fit(b: Bounds, [x, y, w, h]: [f64; 4]) -> (f64, Vec2) {
    let size = Vec2::new((b.max.x - b.min.x).max(1e-6), (b.max.y - b.min.y).max(1e-6));
    let scale = ((w - 2.0 * PAD) / size.x).min((h - 2.0 * PAD) / size.y);
    let offset = Vec2::new(
        x + (w - size.x * scale) / 2.0 - b.min.x * scale,
        y + (h - size.y * scale) / 2.0 - b.min.y * scale,
    );
    (scale, offset)
}
fn hex([r, g, b, _]: [u8; 4]) -> String {
    format!("#{r:02x}{g:02x}{b:02x}")
}
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
fn draw(svg: &mut String, parts: &[&EvaluatedPart], (scale, offset): (f64, Vec2)) {
    let polygon = |svg: &mut String, part: &EvaluatedPart, fill: &str, extra: &str| {
        let points: Vec<String> = part
            .points
            .iter()
            .map(|p| {
                format!(
                    "{:.2},{:.2}",
                    offset.x + p.x * scale,
                    offset.y + p.y * scale
                )
            })
            .collect();
        let _ = writeln!(
            svg,
            r#"<polygon points="{}" fill="{fill}"{extra}/>"#,
            points.join(" ")
        );
    };
    for part in parts {
        let outline = part.outline.map(|o| {
            format!(
                r#" stroke="{}" stroke-opacity="{:.3}" stroke-width="{:.2}" stroke-linejoin="round""#,
                hex(o.color),
                f64::from(o.color[3]) / 255.0,
                o.width * scale
            )
        });
        if let (Some(stroke), Some(OutlineMode::Behind)) = (&outline, part.outline.map(|o| o.mode))
        {
            polygon(svg, part, "none", stroke);
        }
        let opacity = format!(r#" fill-opacity="{:.3}""#, f64::from(part.color[3]) / 255.0);
        let over = match (&outline, part.outline.map(|o| o.mode)) {
            (Some(stroke), Some(OutlineMode::Over)) => format!("{opacity}{stroke}"),
            _ => opacity,
        };
        polygon(svg, part, &hex(part.color), &over);
    }
}

/// Render `item` in every state, held in `hand` of `figure` at rest pose.
pub fn prop_card_svg(
    item: &HandPropItem,
    figure: &RigDefinition,
    hand: HandSide,
) -> Result<PropCard, PropError> {
    let mut rig = figure.clone();
    attach_hand_prop(&mut rig, hand, item, GripPlacement::default())?;
    let parts = rig.evaluate_parts().map_err(PropError::Rig)?;
    let prefix = held_part_id(hand, &item.id, "");
    let figure_bounds = bounds(&parts).ok_or(PropError::Part)?;
    let item_bounds =
        bounds(parts.iter().filter(|p| p.id.starts_with(&prefix))).ok_or(PropError::Part)?;
    let states = item.state_ids();
    let width = PROP_CARD_CELL_WIDTH * states.len() as u32;
    let height = PROP_CARD_HEADER_HEIGHT + PROP_CARD_CELL_HEIGHT;
    let cell_w = f64::from(PROP_CARD_CELL_WIDTH);
    let panel_h = f64::from(PROP_CARD_CELL_HEIGHT) - LABEL;
    let mut svg = String::new();
    let _ = writeln!(
        svg,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}" font-family="sans-serif">"#
    );
    let _ = writeln!(
        svg,
        r##"<rect width="{width}" height="{height}" fill="#f4f1e9"/>"##
    );
    let _ = writeln!(
        svg,
        r##"<text x="16" y="31" font-size="20" fill="#24282d">{} ({}) · {} state{}</text>"##,
        escape(&item.name),
        escape(&item.id),
        states.len(),
        if states.len() == 1 { "" } else { "s" }
    );
    let figure_height = figure_bounds.max.y - figure_bounds.min.y;
    for (column, state) in states.iter().enumerate() {
        let x = column as f64 * cell_w;
        let y = f64::from(PROP_CARD_HEADER_HEIGHT);
        let hidden: Vec<String> = item
            .hidden_parts(state)
            .map_err(|e| PropError::States {
                item: item.id.clone(),
                reason: e.to_string(),
            })?
            .iter()
            .map(|part| held_part_id(hand, &item.id, part))
            .collect();
        let visible: Vec<&EvaluatedPart> =
            parts.iter().filter(|p| !hidden.contains(&p.id)).collect();
        let prop_only: Vec<&EvaluatedPart> = visible
            .iter()
            .copied()
            .filter(|p| p.id.starts_with(&prefix))
            .collect();
        let _ = writeln!(svg, r#"<g id="state-{state}">"#);
        let _ = writeln!(
            svg,
            r##"<rect x="{:.2}" y="{y:.2}" width="{:.2}" height="{}" fill="none" stroke="#c9c2b4"/>"##,
            x + 0.5,
            cell_w - 1.0,
            PROP_CARD_CELL_HEIGHT
        );
        let _ = writeln!(
            svg,
            r##"<text x="{:.2}" y="{:.2}" font-size="16" fill="#24282d">{}</text>"##,
            x + 12.0,
            y + 22.0,
            escape(state)
        );
        let figure_panel = [x, y + LABEL, cell_w * 0.5, panel_h];
        let figure_fit = fit(figure_bounds, figure_panel);
        draw(&mut svg, &visible, figure_fit);
        // Height ruler at the figure's scale.
        let (scale, offset) = figure_fit;
        let ruler_x = x + 8.0;
        let top = offset.y + figure_bounds.min.y * scale;
        let bottom = offset.y + figure_bounds.max.y * scale;
        let _ = writeln!(
            svg,
            r##"<line x1="{ruler_x:.2}" y1="{top:.2}" x2="{ruler_x:.2}" y2="{bottom:.2}" stroke="#8a8478"/>"##
        );
        let _ = writeln!(
            svg,
            r##"<text x="{:.2}" y="{:.2}" font-size="11" fill="#8a8478">{figure_height:.0} u</text>"##,
            ruler_x + 3.0,
            top + 12.0
        );
        let close_panel = [x + cell_w * 0.5, y + LABEL, cell_w * 0.5, panel_h];
        let close_fit = fit(item_bounds, close_panel);
        draw(&mut svg, &prop_only, close_fit);
        let _ = writeln!(
            svg,
            r##"<text x="{:.2}" y="{:.2}" font-size="12" fill="#8a8478">×{:.1}</text>"##,
            x + cell_w * 0.5 + 8.0,
            y + LABEL + panel_h - 10.0,
            close_fit.0 / scale
        );
        svg.push_str("</g>\n");
    }
    svg.push_str("</svg>\n");
    Ok(PropCard { svg, width, height })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn figure() -> RigDefinition {
        let project: serde_json::Value = serde_json::from_str(include_str!(
            "../../../examples/stick-studio/characters/veikko.kilsat"
        ))
        .unwrap();
        let source: serde_json::Value =
            serde_json::from_str(project["source"].as_str().unwrap()).unwrap();
        serde_json::from_value(source["characters"]["VEIKKO"]["rig"].clone()).unwrap()
    }
    fn item(id: &str) -> HandPropItem {
        crate::stick_studio_hand_props()
            .unwrap()
            .items
            .iter()
            .find(|i| i.id == id)
            .cloned()
            .unwrap()
    }
    fn polygons(svg: &str, state: &str) -> usize {
        let start = svg.find(&format!(r#"<g id="state-{state}">"#)).unwrap();
        let end = start + svg[start..].find("</g>").unwrap();
        svg[start..end].matches("<polygon").count()
    }

    #[test]
    fn card_has_one_cell_per_state_and_fixed_size() {
        let cup = item("kahvikuppi");
        let card = prop_card_svg(&cup, &figure(), HandSide::Right).unwrap();
        assert_eq!((card.width, card.height), (2 * 360, 48 + 440));
        assert!(card
            .svg
            .starts_with(r#"<svg xmlns="http://www.w3.org/2000/svg" width="720" height="488""#));
        // `empty` hides the coffee in both the figure view and the close-up.
        assert_eq!(
            polygons(&card.svg, "full"),
            polygons(&card.svg, "empty") + 2
        );
        // Deterministic: same input, same bytes.
        assert_eq!(
            card,
            prop_card_svg(&cup, &figure(), HandSide::Right).unwrap()
        );
    }

    #[test]
    fn items_without_states_get_one_default_cell() {
        let key = item("avain");
        let card = prop_card_svg(&key, &figure(), HandSide::Left).unwrap();
        assert_eq!(card.width, 360);
        assert!(card.svg.contains(r#"<g id="state-default">"#));
        assert!(card.svg.contains("1 state<"));
    }

    #[test]
    fn every_catalog_item_renders() {
        let figure = figure();
        for item in &crate::stick_studio_hand_props().unwrap().items {
            let card = prop_card_svg(item, &figure, HandSide::Right).unwrap();
            assert!(card.svg.ends_with("</svg>\n"), "{}", item.id);
            assert!(
                !card.svg.contains("NaN") && !card.svg.contains("inf"),
                "{}",
                item.id
            );
        }
    }
}
