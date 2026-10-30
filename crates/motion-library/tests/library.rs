//! Schema, reference, licence, assembly and style tests over temporary packs.
use motion_art::style::{check_background, BackgroundStyle};
use motion_art::{Content, Plane, Point, Rect};
use motion_library::manifest::ManifestError;
use motion_library::room::{self, DoorSide, RoomSpec, TimeOfDay};
use motion_library::{
    assemble_location, assemble_set, Anchor, AssembleError, AssetId, Kind, Library, LibraryError,
    Manifest, Policy, Query,
};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static COUNTER: AtomicU64 = AtomicU64::new(0);

struct Pack(PathBuf);
impl Pack {
    fn new() -> Pack {
        let p = std::env::temp_dir().join(format!(
            "motion-library-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&p);
        for k in Kind::ALL {
            std::fs::create_dir_all(p.join("library").join(k.directory())).unwrap();
        }
        Pack(p)
    }
    fn put(&self, m: &Value) -> PathBuf {
        let id = m["id"].as_str().unwrap();
        let kind = Kind::from_prefix(id.split('.').next().unwrap()).unwrap();
        let path = self
            .0
            .join("library")
            .join(kind.directory())
            .join(format!("{id}.json"));
        std::fs::write(&path, serde_json::to_vec_pretty(m).unwrap()).unwrap();
        path
    }
    fn load(
        &self,
        policy: Policy,
    ) -> Result<(Library, Vec<motion_library::Warning>), LibraryError> {
        Library::load(&self.0, policy)
    }
}
impl Drop for Pack {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn license() -> Value {
    json!({"license": "MIT", "author": "Kilsat Studio", "origin": "original", "redistribution": "allowed"})
}
fn color(r: u8, g: u8, b: u8) -> Value {
    json!({"r": r, "g": g, "b": b, "a": 255})
}
fn chair() -> Value {
    json!({
        "schema_version": 1,
        "id": "prop.test.chair_v1",
        "kind": "prop",
        "name": "Test chair",
        "bounds": {"x": 0, "y": 0, "width": 100, "height": 200},
        "depth": "mid",
        "tags": ["chair", "tuoli", "test"],
        "license": license(),
        "palette": {"wood": color(178, 140, 100)},
        "geometry": {"type": "art", "layers": [
            {"id": "seat", "paint": "wood", "content": {"type": "rectangle",
                "rect": {"x": 0, "y": 110, "width": 100, "height": 20}, "color": color(0, 0, 0)}},
            {"id": "leg", "content": {"type": "stroke",
                "points": [{"x": 10, "y": 130}, {"x": 10, "y": 200}], "color": color(36, 40, 45), "width": 6}},
            {"id": "shadow", "depth": "near", "content": {"type": "ellipse",
                "rect": {"x": 0, "y": 190, "width": 100, "height": 10}, "color": color(90, 90, 90)}}
        ]},
        "anchors": [
            {"type": "seat", "id": "seat", "at": {"x": 30, "y": 120}},
            {"type": "ground", "id": "floor", "at": {"x": 0, "y": 200}, "width": 100},
            {"type": "screen", "id": "back", "rect": {"x": 10, "y": 0, "width": 20, "height": 50}}
        ],
        "variants": [
            {"name": "dark", "palette": {"wood": color(110, 90, 70)}},
            {"name": "cushion", "layers": [{"id": "cushion", "content": {"type": "rectangle",
                "rect": {"x": 5, "y": 100, "width": 90, "height": 10}, "color": color(170, 90, 61)}}]}
        ]
    })
}
fn wall_bg() -> Value {
    json!({
        "schema_version": 1,
        "id": "background.test.wall_v1",
        "kind": "background",
        "name": "Test wall",
        "bounds": {"x": 0, "y": 0, "width": 1920, "height": 1080},
        "depth": "far",
        "tags": ["test"],
        "license": license(),
        "geometry": {"type": "art", "layers": [
            {"id": "wall", "content": {"type": "rectangle",
                "rect": {"x": 0, "y": 0, "width": 1920, "height": 1080}, "color": color(220, 216, 206)}},
            {"id": "shelf", "depth": "mid", "content": {"type": "rectangle",
                "rect": {"x": 100, "y": 500, "width": 300, "height": 20}, "color": color(161, 125, 86)}}
        ]},
        "anchors": [
            {"type": "ground", "id": "ground", "at": {"x": 0, "y": 950}, "width": 1920},
            {"type": "doorway", "id": "door", "rect": {"x": 1600, "y": 500, "width": 160, "height": 440}}
        ]
    })
}
fn room_spec() -> RoomSpec {
    RoomSpec {
        width: 1920.0,
        height: 1080.0,
        floor_y: 810.0,
        wall: motion_art::Color {
            r: 226,
            g: 220,
            b: 204,
            a: 255,
        },
        floor: motion_art::Color {
            r: 190,
            g: 172,
            b: 146,
            a: 255,
        },
        windows: 2,
        time_of_day: TimeOfDay::Day,
        door: DoorSide::Right,
    }
}
fn location() -> Value {
    let spec = room_spec();
    json!({
        "schema_version": 1,
        "id": "location.test.room_v1",
        "kind": "location",
        "name": "Test room",
        "bounds": {"x": 0, "y": 0, "width": 1920, "height": 1080},
        "depth": "far",
        "tags": ["test", "room"],
        "license": license(),
        "geometry": {"type": "composite"},
        "room": serde_json::to_value(&spec).unwrap(),
        "variants": serde_json::to_value(room::standard_variants(&spec)).unwrap(),
        "anchors": [{"type": "stand", "id": "centre", "at": {"x": 960, "y": 945}}],
        "parts": [
            {"instance": "chair", "asset": "prop.test.chair_v1", "at": {"x": 500, "y": 745}, "variant": "dark"}
        ]
    })
}
fn set() -> Value {
    json!({
        "schema_version": 1,
        "id": "set.test.room_v1",
        "kind": "set",
        "name": "Test set",
        "bounds": {"x": 0, "y": 0, "width": 1920, "height": 1080},
        "depth": "far",
        "tags": ["test"],
        "license": license(),
        "geometry": {"type": "composite"},
        "location": "location.test.room_v1",
        "parts": [
            {"instance": "small_chair", "asset": "prop.test.chair_v1", "at": {"x": 1000, "y": 800},
             "scale": 0.5, "flip_x": true, "variant": "cushion"}
        ],
        "cameras": [{"id": "wide", "x": 0, "y": 0, "zoom": 1}, {"id": "close", "x": -300, "y": 100, "zoom": 2.4}],
        "marks": [
            {"id": "sit", "kind": "sit", "anchor": "chair/seat"},
            {"id": "sit_small", "kind": "sit", "anchor": "small_chair/seat"},
            {"id": "door", "kind": "stand", "anchor": "room/doorway"},
            {"id": "free", "kind": "stand", "at": {"x": 700, "y": 945}}
        ]
    })
}
fn full_pack() -> Pack {
    let pack = Pack::new();
    for m in [chair(), wall_bg(), location(), set()] {
        pack.put(&m);
    }
    pack
}
fn validate(v: &Value) -> Result<(), ManifestError> {
    let m = Manifest::from_json(&serde_json::to_vec(v).unwrap())?;
    m.validate(Path::new("."))
}

#[test]
fn asset_id_parsing() {
    let id = AssetId::parse("prop.phone.smartphone_v2").unwrap();
    assert_eq!(
        (id.kind, id.category.as_str(), id.name.as_str(), id.version),
        (Kind::Prop, "phone", "smartphone", 2)
    );
    assert_eq!(id.to_string(), "prop.phone.smartphone_v2");
    assert_eq!(AssetId::parse("prop.tv.tv_v1").unwrap().name, "tv");
    for bad in [
        "prop.phone.smartphone",
        "prop.phone.smartphone_v0",
        "prop.phone.smartphone_v01",
        "prop.phone.smartphone_vx",
        "thing.phone.smartphone_v1",
        "prop.Phone.smartphone_v1",
        "prop.phone.smart__phone_v1",
        "prop.phone._v1",
        "prop.phone",
        "prop.a.b.c_v1",
        "prop.phone.smartphone_v99999999999",
    ] {
        assert!(AssetId::parse(bad).is_err(), "{bad} should be rejected");
    }
}

#[test]
fn valid_manifests_pass_and_round_trip() {
    for v in [chair(), wall_bg(), location(), set()] {
        validate(&v).unwrap();
        let m = Manifest::from_json(&serde_json::to_vec(&v).unwrap()).unwrap();
        let again = Manifest::from_json(m.to_json_pretty().unwrap().as_bytes()).unwrap();
        assert_eq!(m, again);
    }
}

fn mutated(base: Value, f: impl FnOnce(&mut Value)) -> Value {
    let mut v = base;
    f(&mut v);
    v
}

#[test]
fn schema_violations_are_typed_errors() {
    use ManifestError as E;
    type Check = fn(&ManifestError) -> bool;
    let cases: Vec<(&str, Value, Check)> = vec![
        (
            "unknown field",
            mutated(chair(), |v| v["extra"] = json!(1)),
            |e| matches!(e, E::Json(_)),
        ),
        (
            "unknown anchor field",
            mutated(chair(), |v| v["anchors"][0]["z"] = json!(1)),
            |e| matches!(e, E::Json(_)),
        ),
        (
            "unknown anchor type",
            mutated(chair(), |v| v["anchors"][0]["type"] = json!("hook")),
            |e| matches!(e, E::Json(_)),
        ),
        (
            "point anchor with rect",
            mutated(
                chair(),
                |v| v["anchors"][0] = json!({"type": "seat", "id": "s", "rect": {"x": 0, "y": 0, "width": 1, "height": 1}}),
            ),
            |e| matches!(e, E::Json(_)),
        ),
        (
            "schema version",
            mutated(chair(), |v| v["schema_version"] = json!(2)),
            |e| matches!(e, E::SchemaVersion { found: 2 }),
        ),
        (
            "bad id",
            mutated(chair(), |v| v["id"] = json!("prop.test.Chair_v1")),
            |e| matches!(e, E::InvalidId { .. }),
        ),
        (
            "kind mismatch",
            mutated(chair(), |v| v["kind"] = json!("background")),
            |e| matches!(e, E::KindMismatch { .. }),
        ),
        (
            "empty name",
            mutated(chair(), |v| v["name"] = json!("  ")),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "no tags",
            mutated(chair(), |v| v["tags"] = json!([])),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "uppercase tag",
            mutated(chair(), |v| v["tags"] = json!(["Chair"])),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "duplicate tag",
            mutated(chair(), |v| v["tags"] = json!(["a", "a"])),
            |e| matches!(e, E::Duplicate { .. }),
        ),
        (
            "bounds",
            mutated(chair(), |v| v["bounds"]["width"] = json!(-1)),
            |e| matches!(e, E::Art { .. }),
        ),
        (
            "art bounds origin",
            mutated(chair(), |v| v["bounds"]["x"] = json!(5)),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "anchor outside",
            mutated(chair(), |v| v["anchors"][0]["at"]["x"] = json!(101)),
            |e| matches!(e, E::OutOfBounds { .. }),
        ),
        (
            "ground too wide",
            mutated(chair(), |v| v["anchors"][1]["width"] = json!(101)),
            |e| matches!(e, E::OutOfBounds { .. }),
        ),
        (
            "ground zero width",
            mutated(chair(), |v| v["anchors"][1]["width"] = json!(0)),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "rect partly outside",
            mutated(chair(), |v| v["anchors"][2]["rect"]["height"] = json!(250)),
            |e| matches!(e, E::OutOfBounds { .. }),
        ),
        (
            "duplicate anchor",
            mutated(chair(), |v| v["anchors"][1]["id"] = json!("seat")),
            |e| matches!(e, E::Duplicate { .. }),
        ),
        (
            "bad anchor id",
            mutated(chair(), |v| v["anchors"][0]["id"] = json!("Seat")),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "duplicate layer",
            mutated(chair(), |v| {
                v["geometry"]["layers"][1]["id"] = json!("seat")
            }),
            |e| matches!(e, E::Duplicate { .. }),
        ),
        (
            "variant layer clashes",
            mutated(chair(), |v| {
                v["variants"][1]["layers"][0]["id"] = json!("seat")
            }),
            |e| matches!(e, E::Duplicate { .. }),
        ),
        (
            "unknown paint",
            mutated(chair(), |v| {
                v["geometry"]["layers"][0]["paint"] = json!("metal")
            }),
            |e| matches!(e, E::UnknownPaletteSlot { .. }),
        ),
        (
            "variant unknown slot",
            mutated(chair(), |v| {
                v["variants"][0]["palette"]["metal"] = color(1, 2, 3)
            }),
            |e| matches!(e, E::UnknownPaletteSlot { .. }),
        ),
        (
            "duplicate variant",
            mutated(chair(), |v| v["variants"][1]["name"] = json!("dark")),
            |e| matches!(e, E::Duplicate { .. }),
        ),
        (
            "image content",
            mutated(
                chair(),
                |v| v["geometry"]["layers"][0]["content"] = json!({"type": "image", "asset_id": "x", "rect": {"x": 0, "y": 0, "width": 1, "height": 1}}),
            ),
            |e| matches!(e, E::UnsupportedContent { .. }),
        ),
        (
            "thin stroke",
            mutated(chair(), |v| {
                v["geometry"]["layers"][1]["content"]["width"] = json!(0)
            }),
            |e| matches!(e, E::Art { .. }),
        ),
        (
            "one-point stroke",
            mutated(chair(), |v| {
                v["geometry"]["layers"][1]["content"]["points"] = json!([{"x": 0, "y": 0}])
            }),
            |e| matches!(e, E::Art { .. }),
        ),
        (
            "empty art",
            mutated(chair(), |v| v["geometry"]["layers"] = json!([])),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "third party without url",
            mutated(chair(), |v| v["license"]["origin"] = json!("third_party")),
            |e| matches!(e, E::License(_)),
        ),
        (
            "bad url",
            mutated(chair(), |v| {
                v["license"]["origin"] = json!("third_party");
                v["license"]["source_url"] = json!("ftp://x")
            }),
            |e| matches!(e, E::License(_)),
        ),
        (
            "bad license",
            mutated(chair(), |v| v["license"]["license"] = json!("")),
            |e| matches!(e, E::License(_)),
        ),
        (
            "room on prop",
            mutated(chair(), |v| {
                v["room"] = serde_json::to_value(room_spec()).unwrap()
            }),
            |e| matches!(e, E::NotAllowed { field: "room", .. }),
        ),
        (
            "parts on prop",
            mutated(chair(), |v| v["parts"] = location()["parts"].clone()),
            |e| matches!(e, E::NotAllowed { field: "parts", .. }),
        ),
        (
            "cameras on prop",
            mutated(chair(), |v| v["cameras"] = set()["cameras"].clone()),
            |e| {
                matches!(
                    e,
                    E::NotAllowed {
                        field: "cameras",
                        ..
                    }
                )
            },
        ),
        (
            "composite prop",
            mutated(chair(), |v| v["geometry"] = json!({"type": "composite"})),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "art location",
            mutated(location(), |v| v["geometry"] = chair()["geometry"].clone()),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "location marks",
            mutated(location(), |v| v["marks"] = set()["marks"].clone()),
            |e| matches!(e, E::NotAllowed { field: "marks", .. }),
        ),
        (
            "location field on location",
            mutated(location(), |v| {
                v["location"] = json!("location.test.room_v1")
            }),
            |e| {
                matches!(
                    e,
                    E::NotAllowed {
                        field: "location",
                        ..
                    }
                )
            },
        ),
        (
            "room too many windows",
            mutated(location(), |v| v["room"]["windows"] = json!(7)),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "room floor",
            mutated(location(), |v| v["room"]["floor_y"] = json!(100)),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "room larger than bounds",
            mutated(location(), |v| v["bounds"]["width"] = json!(1000)),
            |e| matches!(e, E::OutOfBounds { .. }),
        ),
        (
            "variant slot not in room palette",
            mutated(location(), |v| {
                v["variants"][0]["palette"]["neon"] = color(1, 2, 3)
            }),
            |e| matches!(e, E::UnknownPaletteSlot { .. }),
        ),
        (
            "variant layer clashes with room",
            mutated(location(), |v| {
                v["variants"][2]["layers"][0]["id"] = json!("wall")
            }),
            |e| matches!(e, E::Duplicate { .. }),
        ),
        (
            "anchor clashes with room",
            mutated(location(), |v| v["anchors"][0]["id"] = json!("ground")),
            |e| matches!(e, E::Duplicate { .. }),
        ),
        (
            "reserved instance",
            mutated(location(), |v| v["parts"][0]["instance"] = json!("room")),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "duplicate instance",
            mutated(location(), |v| {
                let p = v["parts"][0].clone();
                v["parts"].as_array_mut().unwrap().push(p)
            }),
            |e| matches!(e, E::Duplicate { .. }),
        ),
        (
            "part references location",
            mutated(location(), |v| {
                v["parts"][0]["asset"] = json!("location.test.room_v1")
            }),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "part scale",
            mutated(location(), |v| v["parts"][0]["scale"] = json!(0)),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "set without location",
            mutated(set(), |v| {
                v.as_object_mut().unwrap().remove("location");
            }),
            |e| {
                matches!(
                    e,
                    E::Required {
                        field: "location",
                        ..
                    }
                )
            },
        ),
        (
            "set location kind",
            mutated(set(), |v| v["location"] = json!("prop.test.chair_v1")),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "set anchors",
            mutated(set(), |v| {
                v["anchors"] = json!([{"type": "stand", "id": "a", "at": {"x": 1, "y": 1}}])
            }),
            |e| {
                matches!(
                    e,
                    E::NotAllowed {
                        field: "anchors",
                        ..
                    }
                )
            },
        ),
        (
            "set variants",
            mutated(set(), |v| v["variants"] = json!([{"name": "x"}])),
            |e| {
                matches!(
                    e,
                    E::NotAllowed {
                        field: "variants",
                        ..
                    }
                )
            },
        ),
        (
            "camera zoom",
            mutated(set(), |v| v["cameras"][0]["zoom"] = json!(21)),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "duplicate camera",
            mutated(set(), |v| v["cameras"][1]["id"] = json!("wide")),
            |e| matches!(e, E::Duplicate { .. }),
        ),
        (
            "mark both",
            mutated(set(), |v| v["marks"][0]["at"] = json!({"x": 1, "y": 1})),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "mark neither",
            mutated(set(), |v| {
                v["marks"][3].as_object_mut().unwrap().remove("at");
            }),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "mark anchor format",
            mutated(set(), |v| v["marks"][0]["anchor"] = json!("chair.seat")),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "mark outside",
            mutated(set(), |v| v["marks"][3]["at"]["x"] = json!(5000)),
            |e| matches!(e, E::OutOfBounds { .. }),
        ),
        (
            "blob sha",
            mutated(
                chair(),
                |v| v["geometry"] = json!({"type": "blob", "sha256": "ABC", "bytes": 10, "media_type": "image/png"}),
            ),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "blob media",
            mutated(
                chair(),
                |v| v["geometry"] = json!({"type": "blob", "sha256": "a".repeat(64), "bytes": 10, "media_type": "png"}),
            ),
            |e| matches!(e, E::InvalidField { .. }),
        ),
        (
            "blob size",
            mutated(
                chair(),
                |v| v["geometry"] = json!({"type": "blob", "sha256": "a".repeat(64), "bytes": 0, "media_type": "image/png"}),
            ),
            |e| matches!(e, E::InvalidField { .. }),
        ),
    ];
    for (name, value, check) in cases {
        match validate(&value) {
            Ok(()) => panic!("{name}: expected an error"),
            Err(e) => assert!(check(&e), "{name}: unexpected error {e:?}"),
        }
    }
    // Accepted edge cases.
    validate(&mutated(chair(), |v| {
        v["tags"] = json!(["sänky", "työpöytä_2"])
    }))
    .unwrap();
    validate(&mutated(chair(), |v| v["geometry"] = json!({"type": "blob", "sha256": "a".repeat(64), "bytes": 10, "media_type": "image/png"}))).unwrap();
    validate(&mutated(chair(), |v| {
        v["license"]["origin"] = json!("third_party");
        v["license"]["source_url"] = json!("https://example.org/chair");
        v["license"]["license"] = json!("CC-BY-4.0");
        v["license"]["notice"] = json!("Chair by Example, CC BY 4.0");
    }))
    .unwrap();
    // Ground anchor exactly on the bounds edge with decimal round-off.
    validate(&mutated(chair(), |v| {
        v["bounds"] = json!({"x": 0, "y": 0, "width": 391.262, "height": 356.862});
        v["anchors"] = json!([{"type": "ground", "id": "floor", "at": {"x": 0, "y": 356.862}, "width": 391.262}]);
    }))
    .unwrap();
}

#[test]
fn rig_project_paths_and_hashes() {
    let pack = Pack::new();
    std::fs::create_dir_all(pack.0.join("rigs")).unwrap();
    std::fs::write(pack.0.join("rigs/thing.kilsat"), b"{}").unwrap();
    // sha256("{}")
    let good = "44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a";
    let rig = |path: &str, sha: &str| {
        mutated(chair(), |v| {
            v["geometry"] = json!({"type": "rig_project", "path": path, "sha256": sha});
            v["bounds"] = json!({"x": -50, "y": -50, "width": 200, "height": 300});
            v["anchors"] = json!([]);
            v["variants"] = json!([]);
        })
    };
    let check = |v: &Value| {
        Manifest::from_json(&serde_json::to_vec(v).unwrap())
            .unwrap()
            .validate(&pack.0)
    };
    check(&rig("rigs/thing.kilsat", good)).unwrap();
    assert!(matches!(
        check(&rig("rigs/thing.kilsat", &"0".repeat(64))),
        Err(ManifestError::HashMismatch { .. })
    ));
    for bad in [
        "../outside.kilsat",
        "/etc/hosts",
        "rigs/./thing.kilsat",
        "rigs/missing.kilsat",
        "",
    ] {
        assert!(
            matches!(
                check(&rig(bad, good)),
                Err(ManifestError::RigProject { .. })
            ),
            "{bad}"
        );
    }
    // Symlink escaping the pack.
    let outside = Pack::new();
    std::fs::write(outside.0.join("secret.kilsat"), b"{}").unwrap();
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(
            outside.0.join("secret.kilsat"),
            pack.0.join("rigs/link.kilsat"),
        )
        .unwrap();
        assert!(matches!(
            check(&rig("rigs/link.kilsat", good)),
            Err(ManifestError::RigProject { .. })
        ));
    }
    // A location using a rig prop cannot be kit-bashed into art layers.
    pack.put(&rig("rigs/thing.kilsat", good));
    pack.put(&mutated(location(), |v| {
        v["parts"][0].as_object_mut().unwrap().remove("variant");
    }));
    let (lib, _) = pack.load(Policy::Distribution).unwrap();
    assert!(matches!(
        assemble_location(&lib, "location.test.room_v1", None, "s"),
        Err(AssembleError::UnsupportedGeometry {
            geometry: "rig_project",
            ..
        })
    ));
}

#[test]
fn library_loads_and_searches_deterministically() {
    let pack = full_pack();
    std::fs::write(pack.0.join("library/README.md"), "notes").unwrap();
    let (lib, warnings) = pack.load(Policy::Distribution).unwrap();
    assert!(warnings.is_empty());
    assert_eq!(lib.len(), 4);
    assert_eq!(Kind::ALL.map(|k| lib.count(k)), [1, 1, 1, 1]);
    let ids = |q: &Query| {
        lib.search(q)
            .iter()
            .map(|m| m.id.clone())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        ids(&Query::default()),
        [
            "background.test.wall_v1",
            "location.test.room_v1",
            "prop.test.chair_v1",
            "set.test.room_v1"
        ]
    );
    assert_eq!(
        ids(&Query {
            tags_all: vec!["test".into(), "room".into()],
            ..Default::default()
        }),
        ["location.test.room_v1"]
    );
    assert_eq!(
        ids(&Query {
            text: Some("TUOLI".into()),
            ..Default::default()
        }),
        ["prop.test.chair_v1"]
    );
    assert_eq!(
        ids(&Query {
            kind: Some(Kind::Set),
            text: Some("test".into()),
            ..Default::default()
        }),
        ["set.test.room_v1"]
    );
}

#[test]
fn library_reference_and_layout_errors() {
    let reference = |f: fn(&Pack)| {
        let pack = full_pack();
        f(&pack);
        pack.load(Policy::Authoring).unwrap_err()
    };
    let e = reference(|p| {
        p.put(&mutated(location(), |v| {
            v["parts"][0]["asset"] = json!("prop.test.missing_v1")
        }));
    });
    assert!(
        matches!(e, LibraryError::Reference { ref field, .. } if field == "parts[0].asset"),
        "{e}"
    );
    let e = reference(|p| {
        p.put(&mutated(location(), |v| {
            v["parts"][0]["variant"] = json!("neon")
        }));
    });
    assert!(
        matches!(e, LibraryError::Reference { ref field, .. } if field == "parts[0].variant"),
        "{e}"
    );
    let e = reference(|p| {
        p.put(&mutated(set(), |v| {
            v["location"] = json!("location.test.missing_v1")
        }));
    });
    assert!(
        matches!(e, LibraryError::Reference { ref field, .. } if field == "location"),
        "{e}"
    );
    let e = reference(|p| {
        p.put(&mutated(set(), |v| {
            v["marks"][0]["anchor"] = json!("sofa/seat")
        }));
    });
    assert!(
        matches!(e, LibraryError::Reference { ref field, .. } if field == "marks[0].anchor"),
        "{e}"
    );
    let e = reference(|p| {
        p.put(&mutated(set(), |v| {
            v["marks"][0]["anchor"] = json!("chair/armrest")
        }));
    });
    assert!(matches!(e, LibraryError::Reference { .. }), "{e}");
    let e = reference(|p| {
        p.put(&mutated(set(), |v| {
            v["marks"][0]["anchor"] = json!("room/window")
        }));
    });
    assert!(matches!(e, LibraryError::Reference { .. }), "{e}");
    let e = reference(|p| {
        p.put(&mutated(set(), |v| {
            v["parts"][0]["instance"] = json!("chair")
        }));
    });
    assert!(
        matches!(e, LibraryError::Reference { ref field, .. } if field == "parts[0].instance"),
        "{e}"
    );
    // The error names the file.
    assert!(e.to_string().contains("set.test.room_v1.json"), "{e}");

    let e = reference(|p| {
        let path = p.put(&chair());
        std::fs::rename(&path, path.with_file_name("prop.test.other_v1.json")).unwrap();
    });
    assert!(matches!(e, LibraryError::FileName { .. }), "{e}");
    let e = reference(|p| {
        let path = p.put(&chair());
        std::fs::rename(
            &path,
            p.0.join("library/backgrounds/prop.test.chair_v1.json"),
        )
        .unwrap();
    });
    assert!(matches!(e, LibraryError::Directory { .. }), "{e}");
    let e = reference(|p| {
        std::fs::create_dir_all(p.0.join("library/misc")).unwrap();
    });
    assert!(matches!(e, LibraryError::Layout(_)), "{e}");
    let e = reference(|p| {
        std::fs::create_dir_all(p.0.join("library/props/nested")).unwrap();
    });
    assert!(matches!(e, LibraryError::Layout(_)), "{e}");
    let e = reference(|p| {
        std::fs::write(
            p.0.join("library/props/big.json"),
            vec![b' '; 1024 * 1024 + 1],
        )
        .unwrap();
    });
    assert!(matches!(e, LibraryError::TooLarge { .. }), "{e}");
    let e = reference(|p| {
        std::fs::write(p.0.join("library/props/prop.test.broken_v1.json"), b"{").unwrap();
    });
    assert!(
        matches!(
            e,
            LibraryError::Manifest {
                source: ManifestError::Json(_),
                ..
            }
        ),
        "{e}"
    );
    #[cfg(unix)]
    {
        let e = reference(|p| {
            let path = p.put(&chair());
            let target = p.0.join("chair.json");
            std::fs::rename(&path, &target).unwrap();
            std::os::unix::fs::symlink(&target, &path).unwrap();
        });
        assert!(matches!(e, LibraryError::Symlink(_)), "{e}");
    }
}

#[test]
fn licence_gates_are_transitive() {
    for (field, value) in [
        ("origin", json!("ai_generated")),
        ("license", json!("unknown")),
        ("redistribution", json!("forbidden")),
        ("redistribution", json!("unknown")),
    ] {
        let pack = full_pack();
        pack.put(&mutated(chair(), |v| v["license"][field] = value.clone()));
        let (lib, warnings) = pack.load(Policy::Authoring).unwrap();
        let flagged: Vec<&str> = warnings.iter().map(|w| w.id.as_str()).collect();
        // The prop, the location using it and the set using that location.
        assert_eq!(
            flagged,
            [
                "location.test.room_v1",
                "prop.test.chair_v1",
                "set.test.room_v1"
            ],
            "{field}={value}"
        );
        assert!(lib
            .distribution_blockers("background.test.wall_v1")
            .is_empty());
        assert!(lib.distribution_blockers("set.test.room_v1")[0].contains("prop.test.chair_v1"));
        let e = pack.load(Policy::Distribution).unwrap_err();
        assert!(matches!(e, LibraryError::NotDistributable { .. }), "{e}");
    }
    // Set-only parts propagate too.
    let pack = full_pack();
    let mut bg = wall_bg();
    bg["license"]["origin"] = json!("ai_generated");
    pack.put(&bg);
    pack.put(&mutated(set(), |v| {
        v["parts"][0] = json!({"instance": "backdrop", "asset": "background.test.wall_v1", "at": {"x": 0, "y": 0}});
        v["marks"] = json!([{"id": "door", "kind": "stand", "anchor": "backdrop/door"}]);
    }));
    let (_, warnings) = pack.load(Policy::Authoring).unwrap();
    let flagged: Vec<&str> = warnings.iter().map(|w| w.id.as_str()).collect();
    assert_eq!(flagged, ["background.test.wall_v1", "set.test.room_v1"]);
}

#[test]
fn assembly_is_deterministic_and_ordered() {
    let pack = full_pack();
    let (lib, _) = pack.load(Policy::Distribution).unwrap();
    let a = assemble_set(&lib, "set.test.room_v1", Some("rain"), "s1").unwrap();
    let b = assemble_set(&lib, "set.test.room_v1", Some("rain"), "s1").unwrap();
    assert_eq!(a, b);
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
    // A freshly loaded library gives the same bytes.
    let (lib2, _) = pack.load(Policy::Distribution).unwrap();
    let c = assemble_set(&lib2, "set.test.room_v1", Some("rain"), "s1").unwrap();
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&c).unwrap()
    );

    let layers = &a.document.layers;
    assert!(layers.iter().all(|l| l.scene_id == "s1"));
    assert_eq!(layers[0].id, "s1_room_wall");
    // Background plane first (far, then mid), then foreground; never interleaved.
    let first_fg = layers
        .iter()
        .position(|l| l.plane == Plane::Foreground)
        .unwrap();
    assert!(layers[first_fg..]
        .iter()
        .all(|l| l.plane == Plane::Foreground));
    let pos = |id: &str| layers.iter().position(|l| l.id == id).unwrap();
    // far room layers and rain before mid chair, set part after location part.
    assert!(pos("s1_room_rain_0_0") < pos("s1_chair_seat"));
    assert!(pos("s1_chair_seat") < pos("s1_chair_leg"));
    assert!(pos("s1_chair_leg") < pos("s1_small_chair_seat"));
    assert!(pos("s1_small_chair_cushion") > pos("s1_small_chair_leg"));
    assert_eq!(layers[pos("s1_chair_shadow")].plane, Plane::Foreground);
    // Palette: part variant `dark` recolours `paint: wood`; unpainted keeps its colour.
    match &layers[pos("s1_chair_seat")].content {
        Content::Rectangle { color, .. } => assert_eq!((color.r, color.g, color.b), (110, 90, 70)),
        other => panic!("{other:?}"),
    }
    match &layers[pos("s1_small_chair_seat")].content {
        Content::Rectangle { color, .. } => {
            assert_eq!((color.r, color.g, color.b), (178, 140, 100))
        }
        other => panic!("{other:?}"),
    }
    // Location variant palette: rain glass.
    let glass = |doc: &motion_art::Document| match &doc
        .layers
        .iter()
        .find(|l| l.id.ends_with("window_0_glass"))
        .unwrap()
        .content
    {
        Content::Rectangle { color, .. } => *color,
        other => panic!("{other:?}"),
    };
    let day = assemble_location(&lib, "location.test.room_v1", None, "s1").unwrap();
    assert_ne!(glass(&a.document), glass(&day.document));
    assert_eq!(a.cameras.len(), 2);
    assert!(matches!(
        assemble_location(&lib, "location.test.room_v1", Some("neon"), "s1"),
        Err(AssembleError::UnknownVariant { .. })
    ));
    assert!(matches!(
        assemble_location(&lib, "set.test.room_v1", None, "s1"),
        Err(AssembleError::WrongKind { .. })
    ));
    assert!(matches!(
        assemble_location(&lib, "location.test.nope_v1", None, "s1"),
        Err(AssembleError::UnknownAsset(_))
    ));
    assert!(matches!(
        assemble_location(&lib, "location.test.room_v1", None, " "),
        Err(AssembleError::InvalidScene)
    ));
}

fn anchor<'a>(a: &'a motion_library::Assembled, instance: &str, id: &str) -> &'a Anchor {
    &a.anchors
        .iter()
        .find(|w| w.instance == instance && w.anchor.id() == id)
        .unwrap()
        .anchor
}

#[test]
fn anchor_transforms_scale_and_flip() {
    let pack = full_pack();
    let (lib, _) = pack.load(Policy::Distribution).unwrap();
    let a = assemble_set(&lib, "set.test.room_v1", None, "s").unwrap();
    // Unflipped, scale 1 at (500, 745): world = at + local.
    assert_eq!(
        anchor(&a, "chair", "seat"),
        &Anchor::Seat {
            id: "seat".into(),
            at: Point { x: 530.0, y: 865.0 }
        }
    );
    // Flipped about bounds centre x = 50 and scaled 0.5 at (1000, 800).
    assert_eq!(
        anchor(&a, "small_chair", "seat"),
        &Anchor::Seat {
            id: "seat".into(),
            at: Point {
                x: 1000.0 + 0.5 * 70.0,
                y: 800.0 + 0.5 * 120.0
            }
        }
    );
    assert_eq!(
        anchor(&a, "small_chair", "floor"),
        &Anchor::Ground {
            id: "floor".into(),
            at: Point {
                x: 1000.0,
                y: 900.0
            },
            width: 50.0
        }
    );
    // Rect x' = 2·50 − (10 + 20) = 70.
    assert_eq!(
        anchor(&a, "small_chair", "back"),
        &Anchor::Screen {
            id: "back".into(),
            rect: Rect {
                x: 1035.0,
                y: 800.0,
                width: 10.0,
                height: 25.0
            }
        }
    );
    // Content follows the same transform; strokes scale their width.
    let leg = a
        .document
        .layers
        .iter()
        .find(|l| l.id == "s_small_chair_leg")
        .unwrap();
    match &leg.content {
        Content::Stroke { points, width, .. } => {
            assert_eq!(
                points[0],
                Point {
                    x: 1045.0,
                    y: 865.0
                }
            );
            assert_eq!(*width, 3.0);
        }
        other => panic!("{other:?}"),
    }
    // Room-generated and own anchors are exposed under `room`.
    assert!(matches!(
        anchor(&a, "room", "ground"),
        Anchor::Ground { .. }
    ));
    assert!(matches!(
        anchor(&a, "room", "doorway"),
        Anchor::Doorway { .. }
    ));
    assert!(matches!(anchor(&a, "room", "centre"), Anchor::Stand { .. }));
    // Marks resolve to anchor reference points.
    let mark = |id: &str| a.marks.iter().find(|m| m.id == id).unwrap().at;
    assert_eq!(mark("sit"), Point { x: 530.0, y: 865.0 });
    assert_eq!(
        mark("sit_small"),
        Point {
            x: 1035.0,
            y: 860.0
        }
    );
    assert_eq!(mark("free"), Point { x: 700.0, y: 945.0 });
    let Anchor::Doorway { rect, .. } = anchor(&a, "room", "doorway") else {
        unreachable!()
    };
    assert_eq!(
        mark("door"),
        Point {
            x: rect.x + rect.width / 2.0,
            y: rect.y + rect.height
        }
    );
}

fn style() -> BackgroundStyle {
    BackgroundStyle::stick_studio(1920.0, 1080.0, 1.0)
}

#[test]
fn room_generator_is_deterministic_and_styled() {
    let mut specs = Vec::new();
    for windows in 0..=6 {
        for door in [DoorSide::None, DoorSide::Left, DoorSide::Right] {
            for time_of_day in [TimeOfDay::Day, TimeOfDay::Evening, TimeOfDay::Night] {
                let spec = RoomSpec {
                    windows,
                    door,
                    time_of_day,
                    ..room_spec()
                };
                if spec.validate().is_ok() {
                    specs.push(spec);
                }
            }
        }
    }
    assert!(specs.len() >= 50, "{}", specs.len());
    for spec in &specs {
        assert_eq!(room::generate(spec), room::generate(spec));
        let art = room::generate(spec);
        assert_eq!(art.windows.len(), usize::from(spec.windows));
        assert_eq!(art.door.is_some(), spec.door != DoorSide::None);
        // Windows do not overlap the door or each other.
        let mut rects = art.windows.clone();
        rects.extend(art.door);
        for (i, a) in rects.iter().enumerate() {
            for b in &rects[i + 1..] {
                assert!(a.x + a.width <= b.x || b.x + b.width <= a.x, "{spec:?}");
            }
        }
        let pack = Pack::new();
        let mut loc = location();
        loc["room"] = serde_json::to_value(spec).unwrap();
        loc["variants"] = serde_json::to_value(room::standard_variants(spec)).unwrap();
        loc["parts"] = json!([]);
        pack.put(&loc);
        let (lib, _) = pack.load(Policy::Distribution).unwrap();
        for variant in [
            None,
            Some("day"),
            Some("evening"),
            Some("rain"),
            Some("messy"),
        ] {
            let a = assemble_location(&lib, "location.test.room_v1", variant, "s").unwrap();
            assert_eq!(
                check_background(&a.document, "s", &style()),
                vec![],
                "{spec:?} {variant:?}"
            );
        }
    }
}

#[test]
fn room_validation() {
    let bad = [
        RoomSpec {
            windows: 7,
            ..room_spec()
        },
        RoomSpec {
            floor_y: 300.0,
            ..room_spec()
        },
        RoomSpec {
            floor_y: 1070.0,
            ..room_spec()
        },
        RoomSpec {
            width: f64::NAN,
            ..room_spec()
        },
        RoomSpec {
            height: 10.0,
            ..room_spec()
        },
        RoomSpec {
            width: 200.0,
            windows: 6,
            door: DoorSide::Left,
            ..room_spec()
        },
    ];
    for spec in bad {
        assert!(spec.validate().is_err(), "{spec:?}");
    }
}

/// The shipped stick-studio pack: every manifest valid and distributable,
/// every location × variant and set assembles and passes the background style.
#[test]
fn stick_studio_library_is_valid_and_styled() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/stick-studio");
    let (lib, warnings) =
        Library::load(&root, Policy::Distribution).unwrap_or_else(|e| panic!("{e}"));
    assert!(warnings.is_empty(), "{warnings:?}");
    assert!(
        lib.count(Kind::Location) >= 15,
        "{}",
        lib.count(Kind::Location)
    );
    assert!(lib.count(Kind::Set) >= lib.count(Kind::Location));
    let mut checked = 0;
    for m in lib.iter() {
        match m.kind {
            Kind::Location => {
                for v in
                    std::iter::once(None).chain(m.variants.iter().map(|v| Some(v.name.as_str())))
                {
                    let a = assemble_location(&lib, &m.id, v, "scene")
                        .unwrap_or_else(|e| panic!("{} {v:?}: {e}", m.id));
                    assert_eq!(
                        check_background(&a.document, "scene", &style()),
                        vec![],
                        "{} {v:?}",
                        m.id
                    );
                    checked += 1;
                }
            }
            Kind::Set => {
                let a = assemble_set(&lib, &m.id, None, "scene")
                    .unwrap_or_else(|e| panic!("{}: {e}", m.id));
                assert!(!a.cameras.is_empty() && !a.marks.is_empty(), "{}", m.id);
                for mark in &a.marks {
                    assert!(
                        (0.0..=1920.0).contains(&mark.at.x) && (0.0..=1080.0).contains(&mark.at.y),
                        "{} {}",
                        m.id,
                        mark.id
                    );
                }
                checked += 1;
            }
            _ => {}
        }
    }
    assert!(checked >= 30, "{checked}");
}

#[test]
fn prop_states_are_optional_and_validated() {
    // Existing manifests without `states` still validate and round-trip unchanged.
    let plain = Manifest::from_json(&serde_json::to_vec(&chair()).unwrap()).unwrap();
    assert!(plain.states.is_empty());
    assert!(!plain.to_json_pretty().unwrap().contains("states"));

    let with = |states: Value| mutated(chair(), |v| v["states"] = states);
    let good = with(json!([{"id": "with_shadow"}, {"id": "no_shadow", "hide": ["shadow"]}]));
    validate(&good).unwrap();
    let m = Manifest::from_json(&serde_json::to_vec(&good).unwrap()).unwrap();
    assert_eq!(m.state_ids(), ["with_shadow", "no_shadow"]);
    assert_eq!(m.hidden_layers("no_shadow").unwrap(), ["shadow"]);
    let unknown = m.hidden_layers("broken").unwrap_err().to_string();
    assert!(
        unknown.contains("unknown state `broken`") && unknown.contains("with_shadow, no_shadow"),
        "{unknown}"
    );

    let reason = |states: Value| validate(&with(states)).unwrap_err().to_string();
    assert!(reason(json!([{"id": "a", "hide": ["shadow"]}])).contains("default"));
    assert!(reason(json!([{"id": "a"}, {"id": "b", "hide": ["lid"]}])).contains("`lid`"));
    assert!(reason(json!([{"id": "a"}, {"id": "b", "hide": ["leg", "leg"]}])).contains("once"));
    assert!(reason(json!([{"id": "a"}, {"id": "a"}])).contains("duplicate"));
    assert!(reason(json!([{"id": "A"}])).contains("[a-z0-9_]"));
    assert!(
        reason(json!([{"id": "a"}, {"id": "b", "hide": ["seat", "leg", "shadow"]}]))
            .contains("at least one layer")
    );
    let many: Vec<Value> = (0..9).map(|i| json!({"id": format!("s{i}")})).collect();
    assert!(reason(Value::Array(many)).contains("at most 8"));
    assert!(matches!(
        validate(&with(json!([{"id": "a", "colour": 1}]))),
        Err(ManifestError::Json(_))
    ));
    // Only props carry states.
    let background = mutated(wall_bg(), |v| v["states"] = json!([{"id": "a"}]));
    assert!(matches!(
        validate(&background),
        Err(ManifestError::NotAllowed {
            field: "states",
            ..
        })
    ));
}
