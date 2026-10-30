//! Render contract (ORK-G1-06): `&EvaluatedScene` (+ optional artwork) in, draw
//! commands out. See `docs/render-contract.md`.
//!
//! The renderer owns no clock. Every time-dependent decision (artwork activity,
//! timed clips, lights) reads `EvaluatedScene::time`, which the shared evaluator
//! produced; no render entry point takes a time parameter or keeps state between
//! calls. The same scene therefore renders to identical bytes regardless of
//! seek order, and preview and export share the result.
//!
//! [`paint_order`] reports the exact back-to-front order the SVG renderer
//! (`render_svg*`) and GPU `SceneCommands` use, classified into [`DepthBand`]s.

use motion_scene::EvaluatedScene;

/// Depth bands, back to front. The derived `Ord` is the paint order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DepthBand {
    /// Background-plane artwork (scene-space, unattached or attached).
    Background,
    /// Evaluated actors, in `EvaluatedScene::actors` order.
    Characters,
    /// Foreground-plane artwork attached to an actor (hand-held or worn props).
    Props,
    /// Unattached foreground-plane artwork (scene-space foreground).
    Foreground,
}

/// What a paint item draws; identifiers are stable document/actor IDs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaintSource {
    /// An artwork layer by `Layer::id`.
    ArtLayer(String),
    /// An actor by `ActorPose::id`.
    Actor(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaintItem {
    pub band: DepthBand,
    pub source: PaintSource,
}

/// A later item lies in an earlier band than an item before it; the renderer
/// would paint it on top anyway (it never re-sorts), so this is an authoring issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepthViolation {
    /// Index into the [`paint_order`] result of the offending (later) item.
    pub index: usize,
    pub item: PaintItem,
    /// The highest band painted before `item`.
    pub after_band: DepthBand,
}

/// Exact back-to-front paint order for one evaluated scene.
///
/// Rules (all deterministic; none depends on positions, time of call or hashing):
/// 1. Background-plane artwork in document sibling order (groups flattened in
///    order; layers inactive at `scene.time` are omitted).
/// 2. Actors in `scene.actors` order. The evaluator sorts by composition
///    `layers.z` ascending and breaks ties by character ID, so equal z never
///    depends on input map order or on actor position.
/// 3. Foreground-plane artwork in document sibling order. Attached layers are
///    [`DepthBand::Props`], unattached ones [`DepthBand::Foreground`]. Layers
///    attached to an actor absent from the scene are omitted, as in rendering.
///
/// Scene lights and behind-actor light tints are overlays outside the bands;
/// they are documented in `docs/render-contract.md`.
pub fn paint_order(
    scene: &EvaluatedScene,
    art: Option<(&str, &motion_art::Document)>,
) -> Result<Vec<PaintItem>, crate::Error> {
    let mut items = Vec::with_capacity(scene.actors.len());
    let push_plane = |items: &mut Vec<PaintItem>, plane| -> Result<(), crate::Error> {
        let Some((scene_id, document)) = art else {
            return Ok(());
        };
        let steps = document
            .composition(scene_id, plane, scene.time, Some(scene.camera.angle.name()))
            .map_err(|e| crate::Error::InvalidArtwork(e.to_string()))?;
        for step in steps {
            let motion_art::CompositionStep::Layer(layer) = step else {
                continue;
            };
            if let Some(actor) = &layer.attachment {
                if !scene.actors.iter().any(|a| &a.id == actor) {
                    continue;
                }
            }
            let band = match (plane, layer.attachment.is_some()) {
                (motion_art::Plane::Background, _) => DepthBand::Background,
                (motion_art::Plane::Foreground, true) => DepthBand::Props,
                (motion_art::Plane::Foreground, false) => DepthBand::Foreground,
            };
            items.push(PaintItem {
                band,
                source: PaintSource::ArtLayer(layer.id.clone()),
            });
        }
        Ok(())
    };
    push_plane(&mut items, motion_art::Plane::Background)?;
    items.extend(scene.actors.iter().map(|actor| PaintItem {
        band: DepthBand::Characters,
        source: PaintSource::Actor(actor.id.clone()),
    }));
    push_plane(&mut items, motion_art::Plane::Foreground)?;
    Ok(items)
}

/// Items painted after an item of a later band. Background < Characters is
/// structural; the only reachable violation is a prop listed after unattached
/// foreground artwork, which would draw the prop over the foreground.
pub fn depth_violations(order: &[PaintItem]) -> Vec<DepthViolation> {
    let mut highest: Option<DepthBand> = None;
    let mut out = Vec::new();
    for (index, item) in order.iter().enumerate() {
        match highest {
            Some(after_band) if item.band < after_band => out.push(DepthViolation {
                index,
                item: item.clone(),
                after_band,
            }),
            _ => highest = Some(highest.map_or(item.band, |h| h.max(item.band))),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use motion_art::{Color, Content, Document, Layer, Plane, Rect};

    fn layer(id: &str, plane: Plane, attachment: Option<&str>, rgb: u8) -> Layer {
        Layer {
            id: id.into(),
            scene_id: "s".into(),
            name: id.into(),
            visible: true,
            plane,
            active: None,
            opacity: 1.0,
            attachment: attachment.map(Into::into),
            clip: None,
            clip_shape: None,
            blend: Default::default(),
            content: Content::Rectangle {
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
                color: Color {
                    r: rgb,
                    g: rgb,
                    b: 1,
                    a: 255,
                },
            },
            angle: None,
        }
    }

    fn script(z_a: i32, z_b: i32) -> motion_script::Script {
        motion_script::parse(&format!(
            "project: {{title: d, fps: 24, width: 640, height: 360}}\n\
             characters: {{BEA: {{asset: a}}, ALEX: {{asset: a}}}}\n\
             scenes:\n  - id: s\n    duration: 2\n    composition:\n      layers:\n        \
             ALEX: {{z: {z_a}}}\n        BEA: {{z: {z_b}}}\n    events:\n      \
             - {{type: move, actor: ALEX, at: 0, x: 300, y: 250, duration: 1}}\n      \
             - {{type: move, actor: BEA, at: 0, x: 300, y: 250, duration: 1}}\n"
        ))
        .unwrap()
    }

    fn ids(order: &[PaintItem]) -> Vec<(DepthBand, &str)> {
        order
            .iter()
            .map(|item| match &item.source {
                PaintSource::ArtLayer(id) | PaintSource::Actor(id) => (item.band, id.as_str()),
            })
            .collect()
    }

    fn document() -> Document {
        // Deliberately listed out of band order across planes: planes, not list
        // position, decide background versus foreground.
        Document {
            layers: vec![
                layer("fg", Plane::Foreground, None, 40),
                layer("prop", Plane::Foreground, Some("BEA"), 30),
                layer("bg", Plane::Background, None, 10),
                layer("bg_attached", Plane::Background, Some("ALEX"), 20),
            ],
            ..Default::default()
        }
    }

    #[test]
    fn bands_and_ties_are_documented_order() {
        let scene = motion_scene::evaluate(&script(0, 0), 0, 0.5).unwrap();
        let doc = document();
        let order = paint_order(&scene, Some(("s", &doc))).unwrap();
        assert_eq!(
            ids(&order),
            vec![
                (DepthBand::Background, "bg"),
                (DepthBand::Background, "bg_attached"),
                // Equal z and identical positions: character ID breaks the tie.
                (DepthBand::Characters, "ALEX"),
                (DepthBand::Characters, "BEA"),
                (DepthBand::Foreground, "fg"),
                (DepthBand::Props, "prop"),
            ]
        );
        // The prop listed after unattached foreground is reported, not re-sorted.
        let violations = depth_violations(&order);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].index, 5);
        assert_eq!(violations[0].after_band, DepthBand::Foreground);
    }

    #[test]
    fn z_overrides_id_and_order_is_deterministic_across_seeks() {
        let script = script(0, -1);
        let doc = Document {
            layers: vec![
                layer("bg", Plane::Background, None, 10),
                layer("prop", Plane::Foreground, Some("ALEX"), 30),
                layer("fg", Plane::Foreground, None, 40),
            ],
            ..Default::default()
        };
        let reference = paint_order(
            &motion_scene::evaluate(&script, 0, 0.5).unwrap(),
            Some(("s", &doc)),
        )
        .unwrap();
        assert_eq!(
            ids(&reference),
            vec![
                (DepthBand::Background, "bg"),
                (DepthBand::Characters, "BEA"),
                (DepthBand::Characters, "ALEX"),
                (DepthBand::Props, "prop"),
                (DepthBand::Foreground, "fg"),
            ]
        );
        assert!(depth_violations(&reference).is_empty());
        for time in [1.9, 0.0, 0.5, 1.2, 0.5] {
            let scene = motion_scene::evaluate(&script, 0, time).unwrap();
            assert_eq!(paint_order(&scene, Some(("s", &doc))).unwrap(), reference);
        }
    }

    #[test]
    fn paint_order_matches_svg_painter_order() {
        let scene = motion_scene::evaluate(&script(0, 0), 0, 0.5).unwrap();
        let doc = Document {
            layers: vec![
                layer("bg", Plane::Background, None, 0x10),
                layer("prop", Plane::Foreground, Some("BEA"), 0x30),
                layer("fg", Plane::Foreground, None, 0x40),
            ],
            ..Default::default()
        };
        let order = paint_order(&scene, Some(("s", &doc))).unwrap();
        let svg = crate::render_svg_with_art(&scene, 640, 360, &[], "s", &doc, &Default::default())
            .unwrap();
        // Each paint item has a unique marker in the SVG: art fill or actor label.
        let mut last = 0;
        for item in &order {
            let marker = match &item.source {
                PaintSource::ArtLayer(id) => {
                    let byte = doc.layers.iter().find(|l| &l.id == id).unwrap();
                    let Content::Rectangle { color, .. } = byte.content else {
                        unreachable!()
                    };
                    format!("#{:02x}{:02x}01", color.r, color.g)
                }
                PaintSource::Actor(id) => format!(">{id} · "),
            };
            let at = svg[last..]
                .find(&marker)
                .unwrap_or_else(|| panic!("{marker} missing or out of order"));
            last += at + marker.len();
        }
    }

    #[test]
    fn missing_attachment_target_and_no_art_are_omitted() {
        let scene = motion_scene::evaluate(&script(0, 0), 0, 0.5).unwrap();
        let doc = Document {
            layers: vec![layer("orphan", Plane::Foreground, Some("NOBODY"), 30)],
            ..Default::default()
        };
        let order = paint_order(&scene, Some(("s", &doc))).unwrap();
        assert_eq!(
            ids(&order),
            vec![
                (DepthBand::Characters, "ALEX"),
                (DepthBand::Characters, "BEA")
            ]
        );
        assert_eq!(paint_order(&scene, None).unwrap(), order);
    }
}
