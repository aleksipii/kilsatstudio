//! Manifest schema v1 (ORK-LIB-01) and single-file validation.
//!
//! Every structure denies unknown fields. [`Manifest::validate`] checks one
//! manifest in isolation; references between manifests are checked by
//! [`crate::Library::load`].
use crate::room::{self, RoomSpec};
use motion_art::{Color, Content, Document, Layer, Plane, Point, Rect};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::io::Read;
use std::path::{Component, Path};

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_LOCAL_ID: usize = 64;
pub const MAX_NAME: usize = 256;
pub const MAX_TAGS: usize = 32;
pub const MAX_PALETTE: usize = 64;
pub const MAX_ANCHORS: usize = 128;
pub const MAX_VARIANTS: usize = 16;
/// Most named states one prop may declare.
pub const MAX_PROP_STATES: usize = 8;
pub const MAX_PARTS: usize = 256;
pub const MAX_CAMERAS: usize = 32;
pub const MAX_MARKS: usize = 128;
pub const MAX_RIG_PROJECT_BYTES: u64 = 64 * 1024 * 1024;
pub const MAX_BLOB_BYTES: u64 = 1024 * 1024 * 1024;
pub const MIN_PART_SCALE: f64 = 0.01;
pub const MAX_PART_SCALE: f64 = 100.0;
/// Instance name reserved for a location's own contribution (room, own anchors,
/// location-variant layers).
pub const SELF_INSTANCE: &str = "room";

#[derive(Debug, thiserror::Error)]
pub enum ManifestError {
    #[error("manifest JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("schema_version {found} is not supported (expected {SCHEMA_VERSION})")]
    SchemaVersion { found: u32 },
    #[error("invalid asset id `{id}`: {reason}")]
    InvalidId { id: String, reason: &'static str },
    #[error("kind `{kind}` does not match the prefix of id `{id}`")]
    KindMismatch { id: String, kind: Kind },
    #[error("{field}: {reason}")]
    InvalidField { field: String, reason: String },
    #[error("{field}: lies outside bounds")]
    OutOfBounds { field: String },
    #[error("{field}: duplicate identifier `{id}`")]
    Duplicate { field: String, id: String },
    #[error("{field}: palette slot `{slot}` is not defined")]
    UnknownPaletteSlot { field: String, slot: String },
    #[error("`{field}` is not allowed for kind `{kind}`")]
    NotAllowed { field: &'static str, kind: Kind },
    #[error("`{field}` is required for kind `{kind}`")]
    Required { field: &'static str, kind: Kind },
    #[error("license: {0}")]
    License(String),
    #[error("{field}: {source}")]
    Art {
        field: String,
        source: motion_art::Error,
    },
    #[error(
        "{field}: image content is not supported in library art; use rectangle, ellipse or stroke"
    )]
    UnsupportedContent { field: String },
    #[error("geometry.path `{path}`: {reason}")]
    RigProject { path: String, reason: String },
    #[error("geometry.path `{path}`: sha256 mismatch (manifest {expected}, file {actual})")]
    HashMismatch {
        path: String,
        expected: String,
        actual: String,
    },
}

fn invalid(field: impl Into<String>, reason: impl Into<String>) -> ManifestError {
    ManifestError::InvalidField {
        field: field.into(),
        reason: reason.into(),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Background,
    Prop,
    Location,
    Set,
}
impl Kind {
    pub const ALL: [Kind; 4] = [Kind::Background, Kind::Prop, Kind::Location, Kind::Set];
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Background => "background",
            Kind::Prop => "prop",
            Kind::Location => "location",
            Kind::Set => "set",
        }
    }
    /// Directory under `library/` that holds manifests of this kind.
    pub fn directory(self) -> &'static str {
        match self {
            Kind::Background => "backgrounds",
            Kind::Prop => "props",
            Kind::Location => "locations",
            Kind::Set => "sets",
        }
    }
    pub fn from_prefix(prefix: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.as_str() == prefix)
    }
    pub fn from_directory(dir: &str) -> Option<Kind> {
        Kind::ALL.into_iter().find(|k| k.directory() == dir)
    }
}
impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Parsed `<kind>.<category>.<name>_v<N>`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct AssetId {
    pub kind: Kind,
    pub category: String,
    pub name: String,
    pub version: u32,
}
/// `[a-z0-9]+(_[a-z0-9]+)*`
fn segment(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_LOCAL_ID
        && s.split('_').all(|p| {
            !p.is_empty()
                && p.bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
        })
}
impl AssetId {
    pub fn parse(id: &str) -> Result<AssetId, ManifestError> {
        let fail = |reason| ManifestError::InvalidId {
            id: id.to_owned(),
            reason,
        };
        let parts: Vec<&str> = id.split('.').collect();
        let [kind, category, name_version] = parts[..] else {
            return Err(fail("expected <kind>.<category>.<name>_v<N>"));
        };
        let kind = Kind::from_prefix(kind)
            .ok_or_else(|| fail("prefix must be background, prop, location or set"))?;
        if !segment(category) {
            return Err(fail("category must match [a-z0-9]+(_[a-z0-9]+)*"));
        }
        let (name, version) = name_version
            .rsplit_once("_v")
            .ok_or_else(|| fail("name must end in _v<N>"))?;
        if !segment(name) {
            return Err(fail("name must match [a-z0-9]+(_[a-z0-9]+)*"));
        }
        if version.is_empty()
            || version.starts_with('0')
            || !version.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(fail(
                "version must be a positive integer without leading zeros",
            ));
        }
        let version = version
            .parse::<u32>()
            .map_err(|_| fail("version is too large"))?;
        Ok(AssetId {
            kind,
            category: category.to_owned(),
            name: name.to_owned(),
            version,
        })
    }
}
impl fmt::Display for AssetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}.{}.{}_v{}",
            self.kind, self.category, self.name, self.version
        )
    }
}

/// Default stacking depth. `far` and `mid` map to [`Plane::Background`] (far
/// first), `near` to [`Plane::Foreground`]. Parallax is metadata only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Depth {
    Far,
    Mid,
    Near,
}
impl Depth {
    pub fn plane(self) -> Plane {
        match self {
            Depth::Far | Depth::Mid => Plane::Background,
            Depth::Near => Plane::Foreground,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Origin {
    Original,
    ThirdParty,
    AiGenerated,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Redistribution {
    Allowed,
    Forbidden,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct License {
    /// SPDX expression, `LicenseRef-*` or `unknown`.
    pub license: String,
    pub author: String,
    pub origin: Origin,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_url: Option<String>,
    pub redistribution: Redistribution,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notice: Option<String>,
}
impl License {
    /// Reasons this asset alone may not be distributed; empty when distributable.
    pub fn blockers(&self) -> Vec<&'static str> {
        let mut out = Vec::new();
        if self.origin == Origin::AiGenerated {
            out.push("origin is ai_generated");
        }
        if self.license == "unknown" {
            out.push("license is unknown");
        }
        match self.redistribution {
            Redistribution::Allowed => {}
            Redistribution::Forbidden => out.push("redistribution is forbidden"),
            Redistribution::Unknown => out.push("redistribution is unknown"),
        }
        out
    }
    fn validate(&self) -> Result<(), ManifestError> {
        let l = &self.license;
        let spdx_ok = !l.is_empty()
            && l.len() <= 128
            && l.chars()
                .all(|c| c.is_ascii_alphanumeric() || "-.+:() ".contains(c))
            && !l.trim().is_empty();
        if !spdx_ok {
            return Err(ManifestError::License(
                "license must be an SPDX expression, LicenseRef-* or unknown".into(),
            ));
        }
        if !text_ok(&self.author) {
            return Err(ManifestError::License(
                "author must be non-empty text".into(),
            ));
        }
        if let Some(url) = &self.source_url {
            if !(url.starts_with("https://") || url.starts_with("http://"))
                || url.len() > 2048
                || url.chars().any(|c| c.is_whitespace() || c.is_control())
            {
                return Err(ManifestError::License(
                    "source_url must be an http(s) URL".into(),
                ));
            }
        }
        if self.origin == Origin::ThirdParty && self.source_url.is_none() {
            return Err(ManifestError::License(
                "origin third_party requires source_url".into(),
            ));
        }
        if let Some(notice) = &self.notice {
            if notice.trim().is_empty() || notice.len() > 4096 {
                return Err(ManifestError::License(
                    "notice must be non-empty text up to 4096 bytes".into(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArtLayer {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<Depth>,
    /// Palette slot replacing the content colour at assembly time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paint: Option<String>,
    pub content: Content,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Geometry {
    Art {
        layers: Vec<ArtLayer>,
    },
    RigProject {
        path: String,
        sha256: String,
    },
    Blob {
        sha256: String,
        bytes: u64,
        media_type: String,
    },
    Composite,
}
impl Geometry {
    pub fn type_name(&self) -> &'static str {
        match self {
            Geometry::Art { .. } => "art",
            Geometry::RigProject { .. } => "rig_project",
            Geometry::Blob { .. } => "blob",
            Geometry::Composite => "composite",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Anchor {
    Grip { id: String, at: Point },
    Seat { id: String, at: Point },
    Stand { id: String, at: Point },
    Ground { id: String, at: Point, width: f64 },
    Doorway { id: String, rect: Rect },
    Screen { id: String, rect: Rect },
}
impl Anchor {
    pub fn id(&self) -> &str {
        match self {
            Anchor::Grip { id, .. }
            | Anchor::Seat { id, .. }
            | Anchor::Stand { id, .. }
            | Anchor::Ground { id, .. }
            | Anchor::Doorway { id, .. }
            | Anchor::Screen { id, .. } => id,
        }
    }
    pub fn type_name(&self) -> &'static str {
        match self {
            Anchor::Grip { .. } => "grip",
            Anchor::Seat { .. } => "seat",
            Anchor::Stand { .. } => "stand",
            Anchor::Ground { .. } => "ground",
            Anchor::Doorway { .. } => "doorway",
            Anchor::Screen { .. } => "screen",
        }
    }
    /// Representative standing/sitting point: the point itself, the middle of a
    /// ground segment, or the bottom centre of a rectangle.
    pub fn reference_point(&self) -> Point {
        match self {
            Anchor::Grip { at, .. } | Anchor::Seat { at, .. } | Anchor::Stand { at, .. } => *at,
            Anchor::Ground { at, width, .. } => Point {
                x: at.x + width / 2.0,
                y: at.y,
            },
            Anchor::Doorway { rect, .. } | Anchor::Screen { rect, .. } => Point {
                x: rect.x + rect.width / 2.0,
                y: rect.y + rect.height,
            },
        }
    }
    fn validate(&self, field: &str, bounds: &Rect) -> Result<(), ManifestError> {
        if !local_id(self.id()) {
            return Err(invalid(format!("{field}.id"), "must match [a-z0-9_]{1,64}"));
        }
        let ok = match self {
            Anchor::Grip { at, .. } | Anchor::Seat { at, .. } | Anchor::Stand { at, .. } => {
                finite_point(at)?;
                inside(bounds, *at)
            }
            Anchor::Ground { at, width, .. } => {
                finite_point(at)?;
                if !width.is_finite() || *width <= 0.0 {
                    return Err(invalid(
                        format!("{field}.width"),
                        "must be finite and positive",
                    ));
                }
                inside(bounds, *at)
                    && inside(
                        bounds,
                        Point {
                            x: at.x + width,
                            y: at.y,
                        },
                    )
            }
            Anchor::Doorway { rect, .. } | Anchor::Screen { rect, .. } => {
                rect.validate().map_err(|source| ManifestError::Art {
                    field: format!("{field}.rect"),
                    source,
                })?;
                inside(
                    bounds,
                    Point {
                        x: rect.x,
                        y: rect.y,
                    },
                ) && inside(
                    bounds,
                    Point {
                        x: rect.x + rect.width,
                        y: rect.y + rect.height,
                    },
                )
            }
        };
        if ok {
            Ok(())
        } else {
            Err(ManifestError::OutOfBounds {
                field: field.to_owned(),
            })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Variant {
    pub name: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub palette: BTreeMap<String, Color>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<ArtLayer>,
}

/// One named appearance of a prop, such as `on` / `off` or `full` / `empty`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PropState {
    /// `[a-z0-9_]{1,64}`.
    pub id: String,
    /// `geometry.layers` ids not drawn in this state.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub hide: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Part {
    pub instance: String,
    pub asset: String,
    pub at: Point,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f64>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub flip_x: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub depth: Option<Depth>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

/// Camera framing with MotionScript `CameraKey` semantics: `x`, `y` move the
/// camera centre from the canvas centre in scene pixels; `zoom` 0.05..=20.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CameraSpec {
    pub id: String,
    pub x: f64,
    pub y: f64,
    pub zoom: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarkKind {
    Stand,
    Sit,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Mark {
    pub id: String,
    pub kind: MarkKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<Point>,
    /// `<instance>/<anchor>`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
}
impl Mark {
    /// Split `anchor` into `(instance, anchor id)`.
    pub fn anchor_ref(&self) -> Option<(&str, &str)> {
        self.anchor.as_deref().and_then(|a| a.split_once('/'))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema_version: u32,
    pub id: String,
    pub kind: Kind,
    pub name: String,
    pub bounds: Rect,
    pub depth: Depth,
    pub tags: Vec<String>,
    pub license: License,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub palette: BTreeMap<String, Color>,
    pub geometry: Geometry,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub anchors: Vec<Anchor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variants: Vec<Variant>,
    /// Named prop states (art props only). The first is the default and hides
    /// nothing; the others hide some `geometry.layers` by id.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub states: Vec<PropState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub room: Option<RoomSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parts: Vec<Part>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub cameras: Vec<CameraSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marks: Vec<Mark>,
}

/// `[a-z0-9_]{1,64}`: anchor, layer, instance, variant, palette, camera and mark ids.
pub fn local_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_LOCAL_ID
        && s.bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}
fn text_ok(s: &str) -> bool {
    !s.trim().is_empty() && s.len() <= MAX_NAME && !s.chars().any(char::is_control)
}
fn tag_ok(s: &str) -> bool {
    !s.is_empty()
        && s.chars().count() <= MAX_LOCAL_ID
        && s.chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_' || "äöå".contains(c))
}
/// Absolute tolerance for bounds containment, absorbing decimal round-off such
/// as `172.738 + 184.124 != 356.862` in binary floating point.
pub const BOUNDS_EPSILON: f64 = 1e-6;
fn inside(b: &Rect, p: Point) -> bool {
    let e = BOUNDS_EPSILON;
    p.x >= b.x - e && p.x <= b.x + b.width + e && p.y >= b.y - e && p.y <= b.y + b.height + e
}
fn finite_point(p: &Point) -> Result<(), ManifestError> {
    if p.x.is_finite()
        && p.y.is_finite()
        && p.x.abs() <= motion_art::MAX_COORDINATE
        && p.y.abs() <= motion_art::MAX_COORDINATE
    {
        Ok(())
    } else {
        Err(invalid("point", "coordinates must be finite and bounded"))
    }
}
fn with_field(e: ManifestError, field: &str) -> ManifestError {
    match e {
        ManifestError::InvalidField { reason, .. } => invalid(field, reason),
        other => other,
    }
}
fn sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Lift a library art layer to a document layer (used by validation and assembly).
pub(crate) fn document_layer(id: String, scene_id: &str, plane: Plane, content: Content) -> Layer {
    Layer {
        name: id.clone(),
        id,
        scene_id: scene_id.to_owned(),
        visible: true,
        plane,
        active: None,
        opacity: 1.0,
        attachment: None,
        clip: None,
        clip_shape: None,
        blend: Default::default(),
        content,
    }
}

/// Validate art layers: ids, vector-only content, `motion_art` geometry rules,
/// palette slots and (together) the document budgets.
pub(crate) fn validate_art_layers(
    field: &str,
    layers: &[ArtLayer],
    slots: &BTreeSet<&str>,
    taken: &mut BTreeSet<String>,
) -> Result<(), ManifestError> {
    if layers.len() > motion_art::MAX_LAYERS {
        return Err(invalid(field, "too many layers"));
    }
    let mut doc = Document::default();
    for (i, layer) in layers.iter().enumerate() {
        let f = format!("{field}[{i}]");
        if !local_id(&layer.id) {
            return Err(invalid(format!("{f}.id"), "must match [a-z0-9_]{1,64}"));
        }
        if !taken.insert(layer.id.clone()) {
            return Err(ManifestError::Duplicate {
                field: field.to_owned(),
                id: layer.id.clone(),
            });
        }
        if matches!(
            layer.content,
            Content::Image { .. } | Content::AnimatedImage { .. }
        ) {
            return Err(ManifestError::UnsupportedContent { field: f });
        }
        if let Some(slot) = &layer.paint {
            if !slots.contains(slot.as_str()) {
                return Err(ManifestError::UnknownPaletteSlot {
                    field: format!("{f}.paint"),
                    slot: slot.clone(),
                });
            }
        }
        let l = document_layer(
            layer.id.clone(),
            "validate",
            layer.depth.unwrap_or(Depth::Mid).plane(),
            layer.content.clone(),
        );
        let single = Document {
            layers: vec![l.clone()],
            ..Default::default()
        };
        single.validate().map_err(|source| ManifestError::Art {
            field: format!("{f}.content"),
            source,
        })?;
        doc.layers.push(l);
    }
    doc.validate().map_err(|source| ManifestError::Art {
        field: field.to_owned(),
        source,
    })
}

/// Reject absolute paths, `..`, `.` and empty components; the caller still
/// checks symlink escape after canonicalisation.
pub(crate) fn relative_path_ok(path: &str) -> bool {
    !path.is_empty()
        && !path.contains('\\')
        && !path.contains('\0')
        && path
            .split('/')
            .all(|c| !c.is_empty() && c != "." && c != "..")
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}

impl Manifest {
    /// Declared state ids in order (empty when the prop has no states).
    pub fn state_ids(&self) -> Vec<&str> {
        self.states.iter().map(|s| s.id.as_str()).collect()
    }
    /// `geometry.layers` ids hidden in `state`.
    pub fn hidden_layers(&self, state: &str) -> Result<&[String], ManifestError> {
        self.states
            .iter()
            .find(|s| s.id == state)
            .map(|s| s.hide.as_slice())
            .ok_or_else(|| {
                invalid(
                    "states",
                    format!(
                        "unknown state `{state}`; declared states are {}",
                        if self.states.is_empty() {
                            "none".to_owned()
                        } else {
                            self.state_ids().join(", ")
                        }
                    ),
                )
            })
    }
    fn validate_states(&self) -> Result<(), ManifestError> {
        if self.states.is_empty() {
            return Ok(());
        }
        let Geometry::Art { layers } = &self.geometry else {
            return Err(invalid("states", "only art props can declare states"));
        };
        if self.kind != Kind::Prop {
            return Err(ManifestError::NotAllowed {
                field: "states",
                kind: self.kind,
            });
        }
        if self.states.len() > MAX_PROP_STATES {
            return Err(invalid("states", "at most 8 states"));
        }
        let layer_ids: BTreeSet<&str> = layers.iter().map(|l| l.id.as_str()).collect();
        let mut ids = BTreeSet::new();
        for (index, state) in self.states.iter().enumerate() {
            let field = format!("states[{index}]");
            if !local_id(&state.id) {
                return Err(invalid(field, "id must match [a-z0-9_]{1,64}"));
            }
            if !ids.insert(state.id.as_str()) {
                return Err(ManifestError::Duplicate {
                    field: "states".into(),
                    id: state.id.clone(),
                });
            }
            if index == 0 && !state.hide.is_empty() {
                return Err(invalid(
                    field,
                    "the first state is the default and must hide nothing",
                ));
            }
            let mut hidden = BTreeSet::new();
            for layer in &state.hide {
                if !layer_ids.contains(layer.as_str()) || !hidden.insert(layer.as_str()) {
                    return Err(invalid(
                        format!("{field}.hide"),
                        format!("`{layer}` must name a geometry layer, once"),
                    ));
                }
            }
            if hidden.len() == layer_ids.len() {
                return Err(invalid(field, "must leave at least one layer visible"));
            }
        }
        Ok(())
    }
    pub fn from_json(bytes: &[u8]) -> Result<Manifest, ManifestError> {
        Ok(serde_json::from_slice(bytes)?)
    }
    pub fn to_json_pretty(&self) -> Result<String, ManifestError> {
        let mut s = serde_json::to_string_pretty(self)?;
        s.push('\n');
        Ok(s)
    }
    pub fn asset_id(&self) -> Result<AssetId, ManifestError> {
        AssetId::parse(&self.id)
    }
    pub fn anchor(&self, id: &str) -> Option<&Anchor> {
        self.anchors.iter().find(|a| a.id() == id)
    }
    pub fn variant(&self, name: &str) -> Option<&Variant> {
        self.variants.iter().find(|v| v.name == name)
    }
    /// Generated room art, if this is a location with `room`.
    pub fn room_art(&self) -> Option<room::RoomArt> {
        self.room.as_ref().map(room::generate)
    }

    /// Validate this manifest in isolation. `pack_root` resolves `rig_project` paths.
    pub fn validate(&self, pack_root: &Path) -> Result<(), ManifestError> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(ManifestError::SchemaVersion {
                found: self.schema_version,
            });
        }
        let id = self.asset_id()?;
        if id.kind != self.kind {
            return Err(ManifestError::KindMismatch {
                id: self.id.clone(),
                kind: self.kind,
            });
        }
        let kind = self.kind;
        if !text_ok(&self.name) {
            return Err(invalid("name", "must be non-empty text up to 256 bytes"));
        }
        self.bounds
            .validate()
            .map_err(|source| ManifestError::Art {
                field: "bounds".into(),
                source,
            })?;
        self.validate_tags()?;
        self.license.validate()?;
        if self.palette.len() > MAX_PALETTE {
            return Err(invalid("palette", "too many slots"));
        }
        if let Some(slot) = self.palette.keys().find(|s| !local_id(s)) {
            return Err(invalid(
                format!("palette.{slot}"),
                "slot names must match [a-z0-9_]{1,64}",
            ));
        }

        // Kind gates.
        let composite = matches!(self.geometry, Geometry::Composite);
        match kind {
            Kind::Location | Kind::Set if !composite => {
                return Err(invalid("geometry", "location and set require composite"))
            }
            Kind::Background | Kind::Prop if composite => {
                return Err(invalid(
                    "geometry",
                    "composite is only for location and set",
                ))
            }
            _ => {}
        }
        if self.room.is_some() && kind != Kind::Location {
            return Err(ManifestError::NotAllowed {
                field: "room",
                kind,
            });
        }
        if !self.parts.is_empty() && !matches!(kind, Kind::Location | Kind::Set) {
            return Err(ManifestError::NotAllowed {
                field: "parts",
                kind,
            });
        }
        if kind != Kind::Set {
            if self.location.is_some() {
                return Err(ManifestError::NotAllowed {
                    field: "location",
                    kind,
                });
            }
            if !self.cameras.is_empty() {
                return Err(ManifestError::NotAllowed {
                    field: "cameras",
                    kind,
                });
            }
            if !self.marks.is_empty() {
                return Err(ManifestError::NotAllowed {
                    field: "marks",
                    kind,
                });
            }
        } else {
            if self.location.is_none() {
                return Err(ManifestError::Required {
                    field: "location",
                    kind,
                });
            }
            if !self.anchors.is_empty() {
                return Err(ManifestError::NotAllowed {
                    field: "anchors",
                    kind,
                });
            }
            if !self.variants.is_empty() {
                return Err(ManifestError::NotAllowed {
                    field: "variants",
                    kind,
                });
            }
        }

        // Palette slots available to layers: own palette plus generated room slots.
        let room_art = match &self.room {
            Some(spec) => {
                spec.validate()?;
                let room_rect = Rect {
                    x: 0.0,
                    y: 0.0,
                    width: spec.width,
                    height: spec.height,
                };
                if !inside(&self.bounds, Point { x: 0.0, y: 0.0 })
                    || !inside(
                        &self.bounds,
                        Point {
                            x: room_rect.width,
                            y: room_rect.height,
                        },
                    )
                {
                    return Err(ManifestError::OutOfBounds {
                        field: "room".into(),
                    });
                }
                Some(room::generate(spec))
            }
            None => None,
        };
        let mut slots: BTreeSet<&str> = self.palette.keys().map(String::as_str).collect();
        if let Some(art) = &room_art {
            slots.extend(art.palette.keys().map(String::as_str));
        }

        // Geometry.
        let mut base_ids = BTreeSet::new();
        if let Some(art) = &room_art {
            base_ids.extend(art.layers.iter().map(|l| l.id.clone()));
        }
        match &self.geometry {
            Geometry::Art { layers } => {
                if self.bounds.x != 0.0 || self.bounds.y != 0.0 {
                    return Err(invalid("bounds", "art assets require x = y = 0"));
                }
                if layers.is_empty() {
                    return Err(invalid("geometry.layers", "must not be empty"));
                }
                validate_art_layers("geometry.layers", layers, &slots, &mut base_ids)?;
            }
            Geometry::RigProject { path, sha256 } => {
                if !sha256_hex(sha256) {
                    return Err(invalid(
                        "geometry.sha256",
                        "must be 64 lowercase hex digits",
                    ));
                }
                verify_rig_project(pack_root, path, sha256)?;
            }
            Geometry::Blob {
                sha256,
                bytes,
                media_type,
            } => {
                if !sha256_hex(sha256) {
                    return Err(invalid(
                        "geometry.sha256",
                        "must be 64 lowercase hex digits",
                    ));
                }
                if *bytes == 0 || *bytes > MAX_BLOB_BYTES {
                    return Err(invalid("geometry.bytes", "must be 1..=1 GiB"));
                }
                let mt_ok = media_type.len() <= 128
                    && media_type.split_once('/').is_some_and(|(a, b)| {
                        !a.is_empty()
                            && !b.is_empty()
                            && media_type
                                .bytes()
                                .all(|c| c.is_ascii_graphic() && c != b'\\')
                    });
                if !mt_ok {
                    return Err(invalid("geometry.media_type", "must be type/subtype"));
                }
            }
            Geometry::Composite => {}
        }
        self.validate_states()?;

        // Anchors.
        if self.anchors.len() > MAX_ANCHORS {
            return Err(invalid("anchors", "too many anchors"));
        }
        let mut anchor_ids: BTreeSet<&str> = BTreeSet::new();
        if let Some(art) = &room_art {
            for a in &art.anchors {
                anchor_ids.insert(a.id());
            }
        }
        for (i, anchor) in self.anchors.iter().enumerate() {
            let f = format!("anchors[{i}]");
            anchor
                .validate(&f, &self.bounds)
                .map_err(|e| with_field(e, &f))?;
            if !anchor_ids.insert(anchor.id()) {
                return Err(ManifestError::Duplicate {
                    field: "anchors".into(),
                    id: anchor.id().to_owned(),
                });
            }
        }

        // Variants.
        if self.variants.len() > MAX_VARIANTS {
            return Err(invalid("variants", "too many variants"));
        }
        let mut names = BTreeSet::new();
        for (i, v) in self.variants.iter().enumerate() {
            let f = format!("variants[{i}]");
            if !local_id(&v.name) {
                return Err(invalid(format!("{f}.name"), "must match [a-z0-9_]{1,64}"));
            }
            if !names.insert(v.name.as_str()) {
                return Err(ManifestError::Duplicate {
                    field: "variants".into(),
                    id: v.name.clone(),
                });
            }
            if let Some(slot) = v.palette.keys().find(|s| !slots.contains(s.as_str())) {
                return Err(ManifestError::UnknownPaletteSlot {
                    field: format!("{f}.palette"),
                    slot: slot.clone(),
                });
            }
            let mut taken = base_ids.clone();
            validate_art_layers(&format!("{f}.layers"), &v.layers, &slots, &mut taken)?;
        }

        // Parts.
        if self.parts.len() > MAX_PARTS {
            return Err(invalid("parts", "too many parts"));
        }
        let mut instances = BTreeSet::new();
        for (i, p) in self.parts.iter().enumerate() {
            let f = format!("parts[{i}]");
            if !local_id(&p.instance) || p.instance == SELF_INSTANCE {
                return Err(invalid(
                    format!("{f}.instance"),
                    "must match [a-z0-9_]{1,64} and not be `room`",
                ));
            }
            if !instances.insert(p.instance.as_str()) {
                return Err(ManifestError::Duplicate {
                    field: "parts".into(),
                    id: p.instance.clone(),
                });
            }
            let target = AssetId::parse(&p.asset)?;
            if !matches!(target.kind, Kind::Prop | Kind::Background) {
                return Err(invalid(
                    format!("{f}.asset"),
                    "parts must reference a prop or background",
                ));
            }
            finite_point(&p.at).map_err(|e| with_field(e, &format!("{f}.at")))?;
            if let Some(s) = p.scale {
                if !s.is_finite() || !(MIN_PART_SCALE..=MAX_PART_SCALE).contains(&s) {
                    return Err(invalid(format!("{f}.scale"), "must be 0.01..=100"));
                }
            }
            if p.variant.as_deref().is_some_and(|v| !local_id(v)) {
                return Err(invalid(
                    format!("{f}.variant"),
                    "must match [a-z0-9_]{1,64}",
                ));
            }
        }

        // Set-only fields.
        if let Some(loc) = &self.location {
            if AssetId::parse(loc)?.kind != Kind::Location {
                return Err(invalid("location", "must reference a location"));
            }
        }
        if self.cameras.len() > MAX_CAMERAS {
            return Err(invalid("cameras", "too many cameras"));
        }
        let mut cams = BTreeSet::new();
        for (i, c) in self.cameras.iter().enumerate() {
            let f = format!("cameras[{i}]");
            if !local_id(&c.id) {
                return Err(invalid(format!("{f}.id"), "must match [a-z0-9_]{1,64}"));
            }
            if !cams.insert(c.id.as_str()) {
                return Err(ManifestError::Duplicate {
                    field: "cameras".into(),
                    id: c.id.clone(),
                });
            }
            if !c.x.is_finite()
                || !c.y.is_finite()
                || c.x.abs() > motion_art::MAX_COORDINATE
                || c.y.abs() > motion_art::MAX_COORDINATE
                || !c.zoom.is_finite()
                || !(0.05..=20.0).contains(&c.zoom)
            {
                return Err(invalid(f, "requires finite offsets and zoom 0.05..=20"));
            }
        }
        if self.marks.len() > MAX_MARKS {
            return Err(invalid("marks", "too many marks"));
        }
        let mut marks = BTreeSet::new();
        for (i, m) in self.marks.iter().enumerate() {
            let f = format!("marks[{i}]");
            if !local_id(&m.id) {
                return Err(invalid(format!("{f}.id"), "must match [a-z0-9_]{1,64}"));
            }
            if !marks.insert(m.id.as_str()) {
                return Err(ManifestError::Duplicate {
                    field: "marks".into(),
                    id: m.id.clone(),
                });
            }
            match (&m.at, &m.anchor) {
                (Some(at), None) => {
                    finite_point(at).map_err(|e| with_field(e, &format!("{f}.at")))?;
                    if !inside(&self.bounds, *at) {
                        return Err(ManifestError::OutOfBounds {
                            field: format!("{f}.at"),
                        });
                    }
                }
                (None, Some(_)) => {
                    let ok = m
                        .anchor_ref()
                        .is_some_and(|(inst, a)| local_id(inst) && local_id(a));
                    if !ok {
                        return Err(invalid(
                            format!("{f}.anchor"),
                            "must be <instance>/<anchor>",
                        ));
                    }
                }
                _ => return Err(invalid(f, "exactly one of `at` or `anchor` is required")),
            }
        }
        Ok(())
    }

    fn validate_tags(&self) -> Result<(), ManifestError> {
        if self.tags.is_empty() || self.tags.len() > MAX_TAGS {
            return Err(invalid("tags", "requires 1..=32 tags"));
        }
        let mut seen = BTreeSet::new();
        for tag in &self.tags {
            if !tag_ok(tag) {
                return Err(invalid(
                    "tags",
                    format!("`{tag}` must use lowercase letters (incl. ä/ö/å), digits or _"),
                ));
            }
            if !seen.insert(tag) {
                return Err(ManifestError::Duplicate {
                    field: "tags".into(),
                    id: tag.clone(),
                });
            }
        }
        Ok(())
    }
}

/// Check that a rig project path stays inside the pack (also through symlinks),
/// exists, is bounded and matches its digest.
fn verify_rig_project(pack_root: &Path, path: &str, expected: &str) -> Result<(), ManifestError> {
    let fail = |reason: &str| ManifestError::RigProject {
        path: path.to_owned(),
        reason: reason.to_owned(),
    };
    if !relative_path_ok(path) {
        return Err(fail(
            "must be a relative path inside the pack without `..` or `.`",
        ));
    }
    let root = pack_root
        .canonicalize()
        .map_err(|e| fail(&format!("pack root: {e}")))?;
    let full = root
        .join(path)
        .canonicalize()
        .map_err(|e| fail(&format!("cannot open: {e}")))?;
    if !full.starts_with(&root) {
        return Err(fail("resolves outside the pack (symlink escape)"));
    }
    let meta = std::fs::metadata(&full).map_err(|e| fail(&e.to_string()))?;
    if !meta.is_file() {
        return Err(fail("is not a regular file"));
    }
    if meta.len() > MAX_RIG_PROJECT_BYTES {
        return Err(fail("exceeds 64 MiB"));
    }
    let mut file = std::fs::File::open(&full).map_err(|e| fail(&e.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    let mut total = 0u64;
    loop {
        let n = file.read(&mut buf).map_err(|e| fail(&e.to_string()))?;
        if n == 0 {
            break;
        }
        total += n as u64;
        if total > MAX_RIG_PROJECT_BYTES {
            return Err(fail("exceeds 64 MiB"));
        }
        hasher.update(&buf[..n]);
    }
    let actual = hex(&hasher.finalize());
    if actual != expected {
        return Err(ManifestError::HashMismatch {
            path: path.to_owned(),
            expected: expected.to_owned(),
            actual,
        });
    }
    Ok(())
}
