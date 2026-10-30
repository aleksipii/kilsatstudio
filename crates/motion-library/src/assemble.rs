//! Kit-bash assembly: location/set manifests → one scene's art layers plus
//! world-space anchors, cameras and marks.
//!
//! Stacking: every `far` layer, then every `mid` layer (both
//! [`Plane::Background`]), then every `near` layer ([`Plane::Foreground`]).
//! Within a band the order is: the location's own room/variant layers, the
//! location parts, then the set parts, each part in manifest order and each
//! asset's layers in their declared order (variant layers after base layers).
//! Effective depth: `part.depth`, else `layer.depth`, else the asset's `depth`.
//! Effective parallax: `part.parallax`, else `layer.parallax`, else the
//! effective depth's [`Depth::default_parallax`]. Atmosphere: the set's, else
//! its location's. Both are reported in [`Assembled`] and change nothing until
//! a renderer is given them explicitly.
//! Layer ids are `<scene>_<instance>_<layer>`; the location itself uses the
//! instance name `room`.
//!
//! Transform: `world = at + scale · local'`, where `local'` is the local point
//! mirrored about the vertical centre line of the asset bounds when `flip_x`.
//! Stroke widths scale with `scale`.
//!
//! `rig_project` and `blob` geometry cannot become `motion_art` layers
//! (motion-art has no filled polygon or mesh content), so assembly reports
//! [`AssembleError::UnsupportedGeometry`] instead of skipping them.
use crate::library::Library;
use crate::manifest::{
    document_layer, Anchor, ArtLayer, Atmosphere, CameraSpec, Depth, Geometry, Kind, Manifest,
    MarkKind, Part, SELF_INSTANCE,
};
use motion_art::{Color, Content, Document, Point, Rect};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Debug, thiserror::Error)]
pub enum AssembleError {
    #[error("asset `{0}` does not exist")]
    UnknownAsset(String),
    #[error("asset `{id}` is a {found}, expected {expected}")]
    WrongKind {
        id: String,
        found: Kind,
        expected: Kind,
    },
    #[error("asset `{id}` has no variant `{variant}`")]
    UnknownVariant { id: String, variant: String },
    #[error("asset `{id}` has {geometry} geometry, which cannot be assembled into art layers")]
    UnsupportedGeometry { id: String, geometry: &'static str },
    #[error("asset `{id}`: palette slot `{slot}` is not defined")]
    UnknownPaletteSlot { id: String, slot: String },
    #[error("mark `{mark}`: anchor `{anchor}` does not resolve")]
    UnresolvedMark { mark: String, anchor: String },
    #[error("scene id must be non-empty text without control characters")]
    InvalidScene,
    #[error("assembled document is invalid: {0}")]
    Art(#[from] motion_art::Error),
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorldAnchor {
    pub instance: String,
    /// Asset that declared the anchor (the location itself for `room`).
    pub asset: String,
    /// The anchor with coordinates in world (scene) space.
    pub anchor: Anchor,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WorldMark {
    pub id: String,
    pub kind: MarkKind,
    pub at: Point,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Assembled {
    pub document: Document,
    pub anchors: Vec<WorldAnchor>,
    /// Set cameras (empty for a bare location).
    pub cameras: Vec<CameraSpec>,
    /// Set marks resolved to world points (empty for a bare location).
    pub marks: Vec<WorldMark>,
    /// Effective multiplane parallax factor of every assembled layer, by
    /// document layer id.
    pub parallax: BTreeMap<String, f64>,
    /// Aerial perspective: the set's, else its location's; `None` is off.
    pub atmosphere: Option<Atmosphere>,
}

#[derive(Debug, Clone, Copy)]
struct Transform {
    at: Point,
    scale: f64,
    flip_x: bool,
    centre_x: f64,
}
impl Transform {
    const IDENTITY: Transform = Transform {
        at: Point { x: 0.0, y: 0.0 },
        scale: 1.0,
        flip_x: false,
        centre_x: 0.0,
    };
    fn for_part(part: &Part, bounds: &Rect) -> Transform {
        Transform {
            at: part.at,
            scale: part.scale.unwrap_or(1.0),
            flip_x: part.flip_x,
            centre_x: bounds.x + bounds.width / 2.0,
        }
    }
    fn point(&self, p: Point) -> Point {
        let x = if self.flip_x {
            2.0 * self.centre_x - p.x
        } else {
            p.x
        };
        Point {
            x: self.at.x + self.scale * x,
            y: self.at.y + self.scale * p.y,
        }
    }
    fn rect(&self, r: Rect) -> Rect {
        let x = if self.flip_x {
            2.0 * self.centre_x - (r.x + r.width)
        } else {
            r.x
        };
        Rect {
            x: self.at.x + self.scale * x,
            y: self.at.y + self.scale * r.y,
            width: self.scale * r.width,
            height: self.scale * r.height,
        }
    }
    /// Horizontal segment `[at.x, at.x + width]` at `at.y`.
    fn segment(&self, at: Point, width: f64) -> (Point, f64) {
        let x = if self.flip_x {
            2.0 * self.centre_x - (at.x + width)
        } else {
            at.x
        };
        (
            Point {
                x: self.at.x + self.scale * x,
                y: self.at.y + self.scale * at.y,
            },
            self.scale * width,
        )
    }
    fn anchor(&self, a: &Anchor) -> Anchor {
        match a {
            Anchor::Grip { id, at } => Anchor::Grip {
                id: id.clone(),
                at: self.point(*at),
            },
            Anchor::Seat { id, at } => Anchor::Seat {
                id: id.clone(),
                at: self.point(*at),
            },
            Anchor::Stand { id, at } => Anchor::Stand {
                id: id.clone(),
                at: self.point(*at),
            },
            Anchor::Ground { id, at, width } => {
                let (at, width) = self.segment(*at, *width);
                Anchor::Ground {
                    id: id.clone(),
                    at,
                    width,
                }
            }
            Anchor::Doorway { id, rect } => Anchor::Doorway {
                id: id.clone(),
                rect: self.rect(*rect),
            },
            Anchor::Screen { id, rect } => Anchor::Screen {
                id: id.clone(),
                rect: self.rect(*rect),
            },
        }
    }
    fn content(&self, c: &Content, color: Option<Color>) -> Content {
        match c {
            Content::Rectangle { rect, color: c0 } => Content::Rectangle {
                rect: self.rect(*rect),
                color: color.unwrap_or(*c0),
            },
            Content::Ellipse { rect, color: c0 } => Content::Ellipse {
                rect: self.rect(*rect),
                color: color.unwrap_or(*c0),
            },
            Content::Stroke {
                points,
                color: c0,
                width,
            } => Content::Stroke {
                points: points.iter().map(|p| self.point(*p)).collect(),
                color: color.unwrap_or(*c0),
                width: width * self.scale,
            },
            // Rejected by manifest validation; kept unchanged for completeness.
            other => other.clone(),
        }
    }
}

struct Builder<'a> {
    scene: &'a str,
    bands: [Vec<motion_art::Layer>; 3],
    anchors: Vec<WorldAnchor>,
    parallax: BTreeMap<String, f64>,
}
impl<'a> Builder<'a> {
    fn new(scene: &'a str) -> Builder<'a> {
        Builder {
            scene,
            bands: Default::default(),
            anchors: Vec::new(),
            parallax: BTreeMap::new(),
        }
    }
    #[allow(clippy::too_many_arguments)]
    fn add_layers(
        &mut self,
        asset: &str,
        instance: &str,
        layers: &[&ArtLayer],
        palette: &BTreeMap<String, Color>,
        default_depth: Depth,
        forced_depth: Option<Depth>,
        forced_parallax: Option<f64>,
        t: Transform,
    ) -> Result<(), AssembleError> {
        for layer in layers {
            let depth = forced_depth.or(layer.depth).unwrap_or(default_depth);
            let parallax = forced_parallax
                .or(layer.parallax)
                .unwrap_or(depth.default_parallax());
            let color =
                match &layer.paint {
                    Some(slot) => Some(*palette.get(slot).ok_or_else(|| {
                        AssembleError::UnknownPaletteSlot {
                            id: asset.to_owned(),
                            slot: slot.clone(),
                        }
                    })?),
                    None => None,
                };
            let mut l = document_layer(
                format!("{}_{}_{}", self.scene, instance, layer.id),
                self.scene,
                depth.plane(),
                t.content(&layer.content, color),
            );
            l.name = format!("{instance} {}", layer.id);
            self.parallax.insert(l.id.clone(), parallax);
            let band = match depth {
                Depth::Far => 0,
                Depth::Mid => 1,
                Depth::Near => 2,
            };
            self.bands[band].push(l);
        }
        Ok(())
    }
    fn add_anchors<'b>(
        &mut self,
        asset: &str,
        instance: &str,
        anchors: impl IntoIterator<Item = &'b Anchor>,
        t: Transform,
    ) {
        for a in anchors {
            self.anchors.push(WorldAnchor {
                instance: instance.to_owned(),
                asset: asset.to_owned(),
                anchor: t.anchor(a),
            });
        }
    }
    fn add_part(&mut self, lib: &Library, part: &Part) -> Result<(), AssembleError> {
        let asset = lib
            .get(&part.asset)
            .ok_or_else(|| AssembleError::UnknownAsset(part.asset.clone()))?;
        let Geometry::Art { layers } = &asset.geometry else {
            return Err(AssembleError::UnsupportedGeometry {
                id: asset.id.clone(),
                geometry: asset.geometry.type_name(),
            });
        };
        let variant = match &part.variant {
            Some(v) => Some(
                asset
                    .variant(v)
                    .ok_or_else(|| AssembleError::UnknownVariant {
                        id: asset.id.clone(),
                        variant: v.clone(),
                    })?,
            ),
            None => None,
        };
        let mut palette = asset.palette.clone();
        let mut all: Vec<&ArtLayer> = layers.iter().collect();
        if let Some(v) = variant {
            palette.extend(v.palette.iter().map(|(k, c)| (k.clone(), *c)));
            all.extend(v.layers.iter());
        }
        let t = Transform::for_part(part, &asset.bounds);
        self.add_layers(
            &asset.id,
            &part.instance,
            &all,
            &palette,
            asset.depth,
            part.depth,
            part.parallax,
            t,
        )?;
        self.add_anchors(&asset.id, &part.instance, &asset.anchors, t);
        Ok(())
    }
    fn add_location(
        &mut self,
        lib: &Library,
        loc: &Manifest,
        variant: Option<&str>,
    ) -> Result<(), AssembleError> {
        let variant = match variant {
            Some(v) => Some(
                loc.variant(v)
                    .ok_or_else(|| AssembleError::UnknownVariant {
                        id: loc.id.clone(),
                        variant: v.to_owned(),
                    })?,
            ),
            None => None,
        };
        let room = loc.room_art();
        let mut palette = room.as_ref().map(|r| r.palette.clone()).unwrap_or_default();
        palette.extend(loc.palette.iter().map(|(k, c)| (k.clone(), *c)));
        let mut own: Vec<&ArtLayer> = room.iter().flat_map(|r| r.layers.iter()).collect();
        if let Some(v) = variant {
            palette.extend(v.palette.iter().map(|(k, c)| (k.clone(), *c)));
            own.extend(v.layers.iter());
        }
        self.add_layers(
            &loc.id,
            SELF_INSTANCE,
            &own,
            &palette,
            loc.depth,
            None,
            None,
            Transform::IDENTITY,
        )?;
        let room_anchors = room.iter().flat_map(|r| r.anchors.iter());
        self.add_anchors(
            &loc.id,
            SELF_INSTANCE,
            room_anchors.chain(loc.anchors.iter()),
            Transform::IDENTITY,
        );
        for part in &loc.parts {
            self.add_part(lib, part)?;
        }
        Ok(())
    }
    fn finish(
        self,
        cameras: Vec<CameraSpec>,
        marks: Vec<WorldMark>,
        atmosphere: Option<Atmosphere>,
    ) -> Result<Assembled, AssembleError> {
        let [far, mid, near] = self.bands;
        let document = Document {
            layers: far.into_iter().chain(mid).chain(near).collect(),
            ..Default::default()
        };
        document.validate()?;
        Ok(Assembled {
            document,
            anchors: self.anchors,
            cameras,
            marks,
            parallax: self.parallax,
            atmosphere,
        })
    }
}

fn check_scene(scene_id: &str) -> Result<(), AssembleError> {
    if scene_id.trim().is_empty() || scene_id.len() > 128 || scene_id.chars().any(char::is_control)
    {
        return Err(AssembleError::InvalidScene);
    }
    Ok(())
}

fn expect<'a>(lib: &'a Library, id: &str, kind: Kind) -> Result<&'a Manifest, AssembleError> {
    let m = lib
        .get(id)
        .ok_or_else(|| AssembleError::UnknownAsset(id.to_owned()))?;
    if m.kind != kind {
        return Err(AssembleError::WrongKind {
            id: id.to_owned(),
            found: m.kind,
            expected: kind,
        });
    }
    Ok(m)
}

/// Assemble a location (room, own variant layers and parts) for `scene_id`.
pub fn assemble_location(
    lib: &Library,
    id: &str,
    variant: Option<&str>,
    scene_id: &str,
) -> Result<Assembled, AssembleError> {
    check_scene(scene_id)?;
    let loc = expect(lib, id, Kind::Location)?;
    let mut b = Builder::new(scene_id);
    b.add_location(lib, loc, variant)?;
    b.finish(Vec::new(), Vec::new(), loc.atmosphere)
}

/// Assemble a set: its location (with `variant`), the set's own parts, its
/// cameras and its marks resolved to world points.
pub fn assemble_set(
    lib: &Library,
    id: &str,
    variant: Option<&str>,
    scene_id: &str,
) -> Result<Assembled, AssembleError> {
    check_scene(scene_id)?;
    let set = expect(lib, id, Kind::Set)?;
    let loc_id = set
        .location
        .as_deref()
        .ok_or_else(|| AssembleError::UnknownAsset(String::new()))?;
    let loc = expect(lib, loc_id, Kind::Location)?;
    let mut b = Builder::new(scene_id);
    b.add_location(lib, loc, variant)?;
    for part in &set.parts {
        b.add_part(lib, part)?;
    }
    let mut marks = Vec::new();
    for m in &set.marks {
        let at = match (&m.at, m.anchor_ref()) {
            (Some(at), _) => *at,
            (None, Some((instance, anchor))) => b
                .anchors
                .iter()
                .find(|a| a.instance == instance && a.anchor.id() == anchor)
                .map(|a| a.anchor.reference_point())
                .ok_or_else(|| AssembleError::UnresolvedMark {
                    mark: m.id.clone(),
                    anchor: m.anchor.clone().unwrap_or_default(),
                })?,
            (None, None) => {
                return Err(AssembleError::UnresolvedMark {
                    mark: m.id.clone(),
                    anchor: String::new(),
                })
            }
        };
        marks.push(WorldMark {
            id: m.id.clone(),
            kind: m.kind,
            at,
        });
    }
    b.finish(
        set.cameras.clone(),
        marks,
        set.atmosphere.or(loc.atmosphere),
    )
}
