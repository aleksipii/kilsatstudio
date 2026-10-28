//! Anchored asset library for Kilsat Studio (contract ORK-LIB-01).
//!
//! * [`manifest`] — manifest schema v1 and per-file validation.
//! * [`library`] — loading a pack's `library/` tree, cross-references, licence
//!   gates and deterministic search.
//! * [`store`] — content-addressed store (CAS) for large blobs kept outside Git.
//! * [`assemble`] — kit-bash assembly of locations and sets into a
//!   [`motion_art::Document`] plus world-space anchors, cameras and marks.
//! * [`room`] — deterministic parametric room generator and its standard variants.
//!
//! See `docs/asset-library.md` for the format and its limitations.
pub mod assemble;
pub mod library;
pub mod manifest;
pub mod room;
pub mod store;

pub use assemble::{
    assemble_location, assemble_set, AssembleError, Assembled, WorldAnchor, WorldMark,
};
pub use library::{Library, LibraryError, Policy, Query, Warning};
pub use manifest::{
    Anchor, ArtLayer, AssetId, CameraSpec, Depth, Geometry, Kind, License, Manifest, ManifestError,
    Mark, MarkKind, Origin, Part, Redistribution, Variant, SCHEMA_VERSION,
};
pub use room::{DoorSide, RoomArt, RoomSpec, TimeOfDay};
pub use store::{ContentStore, Digest, StoreError};
