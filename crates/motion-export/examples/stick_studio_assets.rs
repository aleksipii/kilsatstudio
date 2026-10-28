//! Export the original stick studio pack with the shared native scene pipeline.
//! No model, network access or speech provider is needed.
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    io::Read,
    path::{Component, Path, PathBuf},
};

type Result<T> = std::result::Result<T, Box<dyn Error>>;
const MAX_INPUT: u64 = 4 * 1024 * 1024;
const MAX_FILE_OUTPUT: usize = 16 * 1024 * 1024;
const MAX_TOTAL_OUTPUT: usize = 256 * 1024 * 1024;

#[derive(Debug, Deserialize)]
struct Manifest {
    schema_version: u32,
    #[serde(default = "legacy_pack_revision")]
    pack_revision: u32,
    items: Vec<Item>,
}

fn legacy_pack_revision() -> u32 {
    1
}

#[derive(Debug, Clone, Deserialize)]
struct Item {
    id: String,
    name: String,
    kind: String,
    project: String,
    transparent: bool,
    export_width: u32,
    export_height: u32,
    project_sha256: String,
}

#[derive(Serialize)]
struct Exported {
    id: String,
    name: String,
    kind: String,
    project: String,
    declared_project_sha256: String,
    source_sha256_verified_by_this_exporter: bool,
    width: u32,
    height: u32,
    transparent: bool,
    transparent_pixels: usize,
    visible_pixels: usize,
    svg: String,
    png: String,
    svg_bytes: usize,
    png_bytes: usize,
}

#[derive(Serialize)]
struct Report {
    schema_version: u32,
    pack_revision: u32,
    renderer: &'static str,
    source_hash_note: &'static str,
    items: Vec<Exported>,
}

fn safe_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

fn expected_project(item: &Item) -> Result<String> {
    let directory = match item.kind.as_str() {
        "character" => "characters",
        "background" => "backgrounds",
        "prop" => "props",
        _ => return Err("unknown asset kind".into()),
    };
    Ok(format!("{directory}/{}.kilsat", item.id))
}

fn validate_manifest(manifest: &Manifest) -> Result<()> {
    let prop_count = match manifest.pack_revision {
        1 => 14,
        2 => 20,
        _ => return Err("unsupported stick-studio pack revision".into()),
    };
    if manifest.schema_version != 1 || manifest.items.len() != 10 + prop_count {
        return Err("asset manifest schema or revision item count differs".into());
    }
    let mut ids = BTreeSet::new();
    let mut counts = [0usize; 3];
    for item in &manifest.items {
        if !safe_id(&item.id) || !ids.insert(&item.id) {
            return Err("asset IDs must be unique lowercase ASCII identifiers".into());
        }
        if item.name.is_empty()
            || item.name.len() > 128
            || item.name.chars().any(char::is_control)
            || item.project_sha256.len() != 64
            || !item.project_sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || item.project != expected_project(item)?
        {
            return Err("invalid asset name, project path or declared SHA-256".into());
        }
        if item.export_width == 0
            || item.export_height == 0
            || item.export_width > 4096
            || item.export_height > 4096
            || u64::from(item.export_width) * u64::from(item.export_height) > 4_194_304
        {
            return Err("asset dimensions exceed the four-megapixel budget".into());
        }
        let index = match item.kind.as_str() {
            "character" => 0,
            "background" => 1,
            "prop" => 2,
            _ => return Err("unknown asset kind".into()),
        };
        if item.transparent != (index != 1) {
            return Err("characters and props must be cutouts; backgrounds must be opaque".into());
        }
        counts[index] += 1;
    }
    if counts != [6, 4, prop_count] {
        return Err(
            "asset manifest must contain six characters, four backgrounds and its revision prop count"
                .into(),
        );
    }
    Ok(())
}

fn regular_path(root: &Path, relative: &Path) -> Result<PathBuf> {
    if relative.is_absolute()
        || relative
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err("asset path must be a normal relative path".into());
    }
    if fs::symlink_metadata(root)?.file_type().is_symlink() || !root.is_dir() {
        return Err("asset pack must be a regular directory, not a symlink".into());
    }
    let mut path = root.to_owned();
    for component in relative.components() {
        path.push(component);
        if fs::symlink_metadata(&path)?.file_type().is_symlink() {
            return Err("asset paths must not contain symlinks".into());
        }
    }
    if !fs::metadata(&path)?.is_file() {
        return Err("asset input must be a regular file".into());
    }
    Ok(path)
}

fn read_bounded(path: &Path) -> Result<Vec<u8>> {
    let file = fs::File::open(path)?;
    if !file.metadata()?.is_file() || file.metadata()?.len() > MAX_INPUT {
        return Err("asset input exceeds the regular-file / 4 MiB budget".into());
    }
    let mut bytes = Vec::new();
    file.take(MAX_INPUT + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_INPUT {
        return Err("asset input exceeds the 4 MiB budget".into());
    }
    Ok(bytes)
}

fn load_manifest(pack: &Path) -> Result<Manifest> {
    let manifest: Manifest = serde_json::from_slice(&read_bounded(&regular_path(
        pack,
        Path::new("manifest.json"),
    )?)?)?;
    validate_manifest(&manifest)?;
    Ok(manifest)
}

fn render_item(pack: &Path, item: &Item) -> Result<(String, Vec<u8>, usize, usize)> {
    let project = motion_project::decode(&read_bounded(&regular_path(
        pack,
        Path::new(&item.project),
    )?)?)?;
    if !project.assets.is_empty() {
        return Err("asset projects must be self-contained vector projects".into());
    }
    let script = motion_script::parse(&project.source)?;
    if script.project.width != item.export_width || script.project.height != item.export_height {
        return Err("asset export dimensions must match its native project canvas".into());
    }
    let first = script.scenes.first().ok_or("asset project has no scene")?;
    let scene = motion_scene::evaluate(&script, 0, 0.0)?;
    let renderer = if item.transparent {
        motion_render::render_svg_with_art_transparent
    } else {
        motion_render::render_svg_with_art
    };
    let svg = renderer(
        &scene,
        item.export_width,
        item.export_height,
        &[],
        &first.id,
        &project.art,
        &BTreeMap::new(),
    )?;
    if svg.len() > MAX_FILE_OUTPUT {
        return Err("asset SVG exceeds the 16 MiB output budget".into());
    }
    let tree = resvg::usvg::Tree::from_str(&svg, &resvg::usvg::Options::default())?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(item.export_width, item.export_height)
        .ok_or("cannot allocate bounded asset canvas")?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::identity(),
        &mut pixmap.as_mut(),
    );
    let transparent = pixmap
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[3] == 0)
        .count();
    let visible = pixmap
        .data()
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|p| p[3] > 0)
        .count();
    if item.transparent {
        if transparent == 0 || visible == 0 {
            return Err("cutout must contain transparent and visible pixels".into());
        }
    } else if pixmap.data().as_chunks::<4>().0.iter().any(|p| p[3] != 255) {
        return Err("background export must be fully opaque".into());
    }
    let png = pixmap.encode_png()?;
    if png.len() > MAX_FILE_OUTPUT {
        return Err("asset PNG exceeds the 16 MiB output budget".into());
    }
    Ok((svg, png, transparent, visible))
}

fn export(pack: &Path, output: &Path) -> Result<Report> {
    if !pack.is_absolute() || !output.is_absolute() {
        return Err("pack and output must use absolute paths".into());
    }
    let manifest = load_manifest(pack)?;
    // create_dir refuses an existing destination, including a dangling symlink.
    fs::create_dir(output)?;
    let mut items = Vec::new();
    let mut total = 0usize;
    let pack_revision = manifest.pack_revision;
    for item in manifest.items {
        let (svg, png, transparent, visible) = render_item(pack, &item)?;
        total = total
            .checked_add(svg.len() + png.len())
            .filter(|total| *total <= MAX_TOTAL_OUTPUT)
            .ok_or("asset outputs exceed the 256 MiB aggregate budget")?;
        let directory = output.join(&item.kind);
        if !directory.exists() {
            fs::create_dir(&directory)?;
        }
        let svg_path = format!("{}/{}.svg", item.kind, item.id);
        let png_path = format!("{}/{}.png", item.kind, item.id);
        fs::write(output.join(&svg_path), svg.as_bytes())?;
        fs::write(output.join(&png_path), &png)?;
        items.push(Exported {
            id: item.id,
            name: item.name,
            kind: item.kind,
            project: item.project,
            declared_project_sha256: item.project_sha256,
            source_sha256_verified_by_this_exporter: false,
            width: item.export_width,
            height: item.export_height,
            transparent: item.transparent,
            transparent_pixels: transparent,
            visible_pixels: visible,
            svg: svg_path,
            png: png_path,
            svg_bytes: svg.len(),
            png_bytes: png.len(),
        });
    }
    let report = Report {
        schema_version: 1,
        pack_revision,
        renderer: "motion-project → motion-script → motion-scene → motion-render → resvg",
        source_hash_note: "SHA-256 values are declared by the pack manifest; this exporter does not independently verify them. Verify source hashes before packaging.",
        items,
    };
    fs::write(
        output.join("export-report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(report)
}

fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 4 || args[0] != "--pack" || args[2] != "--output" {
        return Err(
            "usage: stick_studio_assets --pack ABSOLUTE_PACK --output NEW_ABSOLUTE_DIRECTORY"
                .into(),
        );
    }
    let report = export(Path::new(&args[1]), Path::new(&args[3]))?;
    println!(
        "Exported {} editable-project previews as native SVG and PNG.",
        report.items.len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/stick-studio")
    }

    #[test]
    fn all_native_projects_decode_evaluate_and_export_correct_alpha() {
        let root = pack();
        let manifest = load_manifest(&root).unwrap();
        for item in &manifest.items {
            let (svg, png, transparent, visible) =
                render_item(&root, item).unwrap_or_else(|e| panic!("{}: {e}", item.id));
            assert!(svg.contains("<svg"));
            assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
            assert!(visible > 0, "{}", item.id);
            assert_eq!(transparent > 0, item.transparent, "{}", item.id);
        }
    }

    fn raster(svg: &str, width: u32, height: u32) -> resvg::tiny_skia::Pixmap {
        let tree = resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).unwrap();
        let mut pixels = resvg::tiny_skia::Pixmap::new(width, height).unwrap();
        resvg::render(
            &tree,
            resvg::tiny_skia::Transform::from_scale(
                width as f32 / tree.size().width(),
                height as f32 / tree.size().height(),
            ),
            &mut pixels.as_mut(),
        );
        pixels
    }

    /// Connected-component checks use the actual exported alpha, with no
    /// dilation or relative-area exception that could hide a detached limb.
    fn alpha_components(pixels: &resvg::tiny_skia::Pixmap) -> Vec<usize> {
        let width = pixels.width() as usize;
        let height = pixels.height() as usize;
        let mut remaining: Vec<bool> = pixels
            .data()
            .as_chunks::<4>()
            .0
            .iter()
            .map(|pixel| pixel[3] >= 32)
            .collect();
        let mut components = Vec::new();
        for start in 0..remaining.len() {
            if !remaining[start] {
                continue;
            }
            remaining[start] = false;
            let mut pending = vec![start];
            let mut count = 0;
            while let Some(index) = pending.pop() {
                count += 1;
                let x = index % width;
                let y = index / width;
                for ny in y.saturating_sub(1)..=(y + 1).min(height - 1) {
                    for nx in x.saturating_sub(1)..=(x + 1).min(width - 1) {
                        let next = ny * width + nx;
                        if remaining[next] {
                            remaining[next] = false;
                            pending.push(next);
                        }
                    }
                }
            }
            components.push(count);
        }
        components.sort_unstable_by(|a, b| b.cmp(a));
        components
    }

    #[test]
    fn all_six_silhouettes_remain_connected_through_bent_joint_poses() {
        use serde_json::json;
        let poses: [&[(&str, f64)]; 7] = [
            &[],
            &[
                ("head", 0.35),
                ("torso", 0.3),
                ("upper_arm_l", 2.3),
                ("forearm_l", -1.1),
            ],
            &[
                ("head", -0.35),
                ("torso", -0.3),
                ("upper_arm_r", -2.3),
                ("forearm_r", 1.1),
            ],
            &[
                ("thigh_l", 0.7),
                ("shin_l", -1.1),
                ("thigh_r", -0.6),
                ("shin_r", 0.9),
            ],
            &[
                ("upper_arm_l", 1.3),
                ("forearm_l", 1.4),
                ("hand_l", -1.7),
                ("upper_arm_r", -1.25),
                ("forearm_r", -2.1),
                ("hand_r", 3.51),
            ],
            &[
                ("torso", 0.45),
                ("upper_arm_l", -1.0),
                ("upper_arm_r", 1.0),
                ("thigh_l", -0.55),
                ("shin_l", 1.0),
            ],
            &[
                ("torso", -0.45),
                ("upper_arm_l", 2.0),
                ("upper_arm_r", -2.0),
                ("thigh_r", 0.55),
                ("shin_r", -1.0),
            ],
        ];
        let root = pack();
        let manifest = load_manifest(&root).unwrap();
        for item in manifest
            .items
            .iter()
            .filter(|item| item.kind == "character")
        {
            let project =
                motion_project::decode(&fs::read(root.join(&item.project)).unwrap()).unwrap();
            let original: serde_json::Value = serde_json::from_str(&project.source).unwrap();
            let actor_id = original["characters"]
                .as_object()
                .unwrap()
                .keys()
                .next()
                .unwrap()
                .clone();
            for (pose_index, offsets) in poses.iter().enumerate() {
                let mut value = original.clone();
                let composition = &mut value["scenes"][0]["composition"];
                composition["camera"] = json!([{"at":0,"x":0,"y":0,"zoom":1}]);
                // Keep even the reaper's long staff inside the test canvas.
                composition["actor_tracks"] =
                    json!({actor_id.clone():[{"at":0,"y":-110,"scale_x":1.1,"scale_y":1.1}]});
                let mut tracks: serde_json::Map<String, serde_json::Value> = offsets
                    .iter()
                    .map(|(joint, rotation)| {
                        ((*joint).to_owned(), json!([{"at":0,"rotation":rotation}]))
                    })
                    .collect();
                if tracks.is_empty() {
                    tracks.insert("root".into(), json!([{"at":0}]));
                }
                composition["joint_tracks"] = json!({actor_id.clone():tracks});
                let script = motion_script::parse(&value.to_string()).unwrap();
                let scene = motion_scene::evaluate(&script, 0, 0.0).unwrap();
                let svg = motion_render::render_svg_with_art_transparent(
                    &scene,
                    item.export_width,
                    item.export_height,
                    &[],
                    &script.scenes[0].id,
                    &project.art,
                    &BTreeMap::new(),
                )
                .unwrap();
                let pixels = raster(&svg, item.export_width, item.export_height);
                let components = alpha_components(&pixels);
                let substantial = components.iter().filter(|count| **count > 4).count();
                let dust: usize = components.iter().filter(|count| **count <= 4).sum();
                assert_eq!(
                    substantial, 1,
                    "{} pose {pose_index}: detached silhouette components {components:?}",
                    item.id
                );
                assert!(
                    dust <= 16,
                    "{} pose {pose_index}: excess detached pixels {dust}",
                    item.id
                );
            }
        }
    }

    #[test]
    fn held_phone_follows_hand_ancestors_root_and_camera_and_matches_native_preview() {
        use serde_json::json;
        use std::sync::Arc;
        let project =
            motion_project::decode(&fs::read(pack().join("stick-studio.kilsat")).unwrap()).unwrap();
        let original: serde_json::Value = serde_json::from_str(&project.source).unwrap();
        for (torso, shoulder, elbow, wrist, root_rotation, sx, sy, cx, cy, zoom) in [
            (0.0, 0.0, 0.0, 0.0, 0.0_f64, 1.0, 1.0, 0.0, 0.0, 1.0),
            (
                0.23, -1.25, -2.1, 3.51, -0.17, 1.6, 1.2, -310.0, -105.0, 2.6,
            ),
            (-0.31, 0.8, 1.0, -0.6, 0.37, 1.1, 1.7, 220.0, -80.0, 1.8),
        ] {
            let offsets = BTreeMap::from([
                ("torso", torso),
                ("upper_arm_r", shoulder),
                ("forearm_r", elbow),
                ("hand_r", wrist),
            ]);
            let mut value = original.clone();
            let composition = &mut value["scenes"][0]["composition"];
            composition["camera"] = json!([{"at":0,"x":cx,"y":cy,"zoom":zoom}]);
            composition["joint_tracks"] = json!({"VEIKKO":offsets.iter().map(|(joint,rotation)| ((*joint).to_owned(),json!([{"at":0,"rotation":rotation}]))).collect::<serde_json::Map<_,_>>()});
            composition["actor_tracks"] = json!({"VEIKKO":[{"at":0,"x":-80,"y":-75,"rotation":root_rotation,"scale_x":sx,"scale_y":sy}]});
            value["scenes"][0]["events"] = json!([]);
            let script = motion_script::parse(&value.to_string()).unwrap();
            let rig = script.characters["VEIKKO"].rig.as_ref().unwrap();
            let held: Vec<_> = rig
                .parts
                .iter()
                .filter(|part| part.id.starts_with("heldprop_r_"))
                .collect();
            assert!(
                held.iter().any(|part| part.id == "heldprop_r_puhelin_case"),
                "starter must demonstrate an actual hand-bound phone"
            );
            assert!(held.iter().all(|part| part.joint == "hand_r"));
            let mut matrices: BTreeMap<&str, motion_rig::Affine2> = BTreeMap::new();
            // Independent parent-by-parent FK expectation, rather than copying
            // the evaluated wrist or another renderer's already posed points.
            for joint in &rig.joints {
                let local = motion_rig::Transform2 {
                    translation: motion_rig::Vec2::new(joint.x, joint.y),
                    rotation: joint.rotation
                        + offsets.get(joint.id.as_str()).copied().unwrap_or(0.0),
                    scale: motion_rig::Vec2::new(joint.scale_x, joint.scale_y),
                }
                .affine();
                let world = joint
                    .parent
                    .as_deref()
                    .map_or(local, |parent| matrices[parent].compose(local));
                matrices.insert(&joint.id, world);
            }
            let scene = motion_scene::evaluate(&script, 0, 0.0).unwrap();
            let actor = scene
                .actors
                .iter()
                .find(|actor| actor.id == "VEIKKO")
                .unwrap();
            for definition in held {
                let evaluated = actor
                    .rigid_parts
                    .iter()
                    .find(|part| part.id == definition.id)
                    .unwrap();
                let points: Vec<[f64; 2]> = definition.ellipse.map_or_else(
                    || definition.points.clone(),
                    |ellipse| {
                        (0..motion_rig::ELLIPSE_SEGMENTS)
                            .map(|index| {
                                let angle = std::f64::consts::TAU * index as f64
                                    / motion_rig::ELLIPSE_SEGMENTS as f64;
                                [
                                    ellipse.cx + ellipse.rx * angle.cos(),
                                    ellipse.cy + ellipse.ry * angle.sin(),
                                ]
                            })
                            .collect()
                    },
                );
                assert_eq!(
                    definition.smooth, 0,
                    "held geometry must preserve its grip outline"
                );
                assert_eq!(points.len(), evaluated.points.len());
                for (point, actual) in points.iter().zip(&evaluated.points) {
                    let expected =
                        matrices["hand_r"].point(motion_rig::Vec2::new(point[0], point[1]));
                    assert!(
                        (expected.x - actual.x).abs() < 1e-8
                            && (expected.y - actual.y).abs() < 1e-8,
                        "{}: phone or fingers lost hand FK",
                        definition.id
                    );
                    let (s, c) = root_rotation.sin_cos();
                    let wx = actor.x + c * sx * expected.x - s * sy * expected.y;
                    let wy = actor.y + s * sx * expected.x + c * sy * expected.y;
                    let world = actor.local_to_world(motion_scene::Vec2::new(actual.x, actual.y));
                    let projected =
                        scene
                            .camera
                            .project(world.x, world.y, scene.width, scene.height);
                    let expected_screen = (
                        960.0 + (wx - 960.0 - cx) * zoom,
                        540.0 + (wy - 540.0 - cy) * zoom,
                    );
                    assert!(
                        (projected.0 - expected_screen.0).abs() < 1e-8
                            && (projected.1 - expected_screen.1).abs() < 1e-8
                    );
                }
            }
            let art = motion_export::ArtContext {
                document: project.art.clone(),
                ..Default::default()
            };
            let preview =
                motion_export::ScenePreview::new(Arc::new(script.clone()), 0, Arc::new(art), None)
                    .unwrap();
            let rgba = preview
                .render(0, 640, &motion_tts::CancellationToken::new())
                .unwrap();
            let svg = motion_render::render_svg_with_art(
                &scene,
                scene.width,
                scene.height,
                &[],
                &script.scenes[0].id,
                &project.art,
                &BTreeMap::new(),
            )
            .unwrap();
            assert_eq!(rgba.rgba, raster(&svg, rgba.width, rgba.height).data());
        }
    }

    #[test]
    fn pack_revisions_are_explicit_and_legacy_asset_exports_still_work() {
        let original: serde_json::Value =
            serde_json::from_slice(&fs::read(pack().join("manifest.json")).unwrap()).unwrap();
        let mut legacy = original.clone();
        legacy.as_object_mut().unwrap().remove("pack_revision");
        let mut props = 0;
        legacy["items"].as_array_mut().unwrap().retain(|item| {
            if item["kind"] == "prop" {
                props += 1;
                props <= 14
            } else {
                true
            }
        });
        let parsed: Manifest = serde_json::from_value(legacy).unwrap();
        assert_eq!(parsed.pack_revision, 1);
        validate_manifest(&parsed).unwrap();
        for revision in [0, 3, u32::MAX] {
            let mut invalid = original.clone();
            invalid["pack_revision"] = serde_json::json!(revision);
            assert!(validate_manifest(&serde_json::from_value(invalid).unwrap()).is_err());
        }
    }

    #[test]
    fn all_six_character_projects_map_speech_and_change_only_mouth_pixels() {
        use motion_lipsync::Viseme;
        let root = pack();
        let manifest = load_manifest(&root).unwrap();
        let mut checked = 0;
        for item in manifest
            .items
            .iter()
            .filter(|item| item.kind == "character")
        {
            let project = motion_project::decode(
                &read_bounded(&regular_path(&root, Path::new(&item.project)).unwrap()).unwrap(),
            )
            .unwrap();
            let script = motion_script::parse(&project.source).unwrap();
            assert_eq!(script.characters.len(), 1, "{}", item.id);
            let (actor_id, character) = script.characters.first_key_value().unwrap();
            let map = character.rig.as_ref().unwrap().visemes.as_ref().unwrap();
            assert_eq!(map.map.len(), motion_rig::VISEME_NAMES.len(), "{}", item.id);
            assert!(
                motion_rig::VISEME_NAMES
                    .iter()
                    .all(|name| map.map.contains_key(*name)),
                "{}",
                item.id
            );

            let pose = motion_scene::evaluate(&script, 0, 0.0).unwrap();
            assert_eq!(pose.actors.len(), 1, "{}", item.id);
            let mouth = pose.actors[0]
                .mouth
                .as_ref()
                .expect("cast member must expose evaluated mouth variants");
            assert_eq!(
                mouth.shapes.len(),
                motion_rig::VISEME_NAMES.len(),
                "{}",
                item.id
            );
            for viseme in [Viseme::Rest, Viseme::A] {
                assert!(
                    mouth.shapes.iter().any(|(v, _)| *v == viseme),
                    "{}: {viseme:?}",
                    item.id
                );
            }
            let mut rest = pose.clone();
            motion_scene::apply_speech(&mut rest, &[(actor_id.as_str(), Viseme::Rest)]);
            let mut open = pose.clone();
            motion_scene::apply_speech(&mut open, &[(actor_id.as_str(), Viseme::A)]);
            for (index, (a, b)) in rest.actors[0]
                .rigid_parts
                .iter()
                .zip(&open.actors[0].rigid_parts)
                .enumerate()
            {
                if index == mouth.part {
                    assert_ne!(a, b, "{}: open mouth must differ from rest", item.id);
                } else {
                    assert_eq!(a, b, "{}: speech changed a non-mouth part", item.id);
                }
            }
            let render = |scene: &motion_scene::EvaluatedScene, viseme| {
                motion_render::render_svg_with_art_transparent(
                    scene,
                    item.export_width,
                    item.export_height,
                    &[(actor_id.as_str(), viseme)],
                    &script.scenes[0].id,
                    &project.art,
                    &BTreeMap::new(),
                )
                .unwrap()
            };
            let rest_svg = render(&rest, Viseme::Rest);
            let open_svg = render(&open, Viseme::A);
            assert_ne!(
                rest_svg, open_svg,
                "{}: mouth variant must reach SVG",
                item.id
            );
            let raster = |svg: &str| {
                let tree =
                    resvg::usvg::Tree::from_str(svg, &resvg::usvg::Options::default()).unwrap();
                let mut pixels =
                    resvg::tiny_skia::Pixmap::new(item.export_width, item.export_height).unwrap();
                resvg::render(
                    &tree,
                    resvg::tiny_skia::Transform::identity(),
                    &mut pixels.as_mut(),
                );
                pixels
            };
            let rest_pixels = raster(&rest_svg);
            let open_pixels = raster(&open_svg);
            // Bound every changed pixel to the two mouth shapes, allowing three
            // pixels for outlines and antialiasing. Other pose parts remain fixed.
            let mut bounds = [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ];
            for scene in [&rest, &open] {
                let actor = &scene.actors[0];
                for point in &actor.rigid_parts[mouth.part].points {
                    let world = actor.local_to_world(motion_scene::Vec2::new(point.x, point.y));
                    let (x, y) = scene
                        .camera
                        .project(world.x, world.y, scene.width, scene.height);
                    bounds[0] = bounds[0].min(x);
                    bounds[1] = bounds[1].min(y);
                    bounds[2] = bounds[2].max(x);
                    bounds[3] = bounds[3].max(y);
                }
            }
            let mut changed = 0;
            for (pixel, (a, b)) in rest_pixels
                .data()
                .as_chunks::<4>()
                .0
                .iter()
                .zip(open_pixels.data().as_chunks::<4>().0)
                .enumerate()
            {
                if a != b {
                    changed += 1;
                    let x = (pixel % item.export_width as usize) as f64 + 0.5;
                    let y = (pixel / item.export_width as usize) as f64 + 0.5;
                    assert!(
                        x >= bounds[0] - 3.0
                            && x <= bounds[2] + 3.0
                            && y >= bounds[1] - 3.0
                            && y <= bounds[3] + 3.0,
                        "{}: changed pixel ({x}, {y}) lies outside mouth bounds {bounds:?}",
                        item.id
                    );
                }
            }
            assert!(
                changed > 0,
                "{}: mouth change must be visible in actual RGBA pixels",
                item.id
            );
            checked += 1;
        }
        assert_eq!(checked, 6);
    }

    #[test]
    fn manifest_rejects_unsafe_paths_unbounded_canvases_and_wrong_transparency() {
        let valid = fs::read(pack().join("manifest.json")).unwrap();
        let parse = || serde_json::from_slice::<Manifest>(&valid).unwrap();
        let mut manifest = parse();
        manifest.items[0].id = "../outside".into();
        assert!(validate_manifest(&manifest).is_err());
        let mut manifest = parse();
        manifest.items[0].project = "/tmp/outside.kilsat".into();
        assert!(validate_manifest(&manifest).is_err());
        let mut manifest = parse();
        manifest.items[0].export_width = u32::MAX;
        assert!(validate_manifest(&manifest).is_err());
        let mut manifest = parse();
        manifest.items[0].transparent = !manifest.items[0].transparent;
        assert!(validate_manifest(&manifest).is_err());
        let mut manifest = parse();
        manifest.items.remove(0);
        assert!(validate_manifest(&manifest).is_err());
        assert!(regular_path(&pack(), Path::new("../manifest.json")).is_err());
    }

    #[test]
    fn file_inputs_reject_oversized_files_and_symlinks() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("stick-asset-input-{}-{unique}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let input = root.join("oversized.kilsat");
        fs::File::create(&input)
            .unwrap()
            .set_len(MAX_INPUT + 1)
            .unwrap();
        assert!(read_bounded(&input).is_err());
        assert!(regular_path(&root, Path::new(".")).is_err());
        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(&input, root.join("linked.kilsat")).unwrap();
            assert!(regular_path(&root, Path::new("linked.kilsat")).is_err());
            std::os::unix::fs::symlink(&root, root.join("linked-directory")).unwrap();
            assert!(regular_path(&root, Path::new("linked-directory/oversized.kilsat")).is_err());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
