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
    items: Vec<Item>,
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
    if manifest.schema_version != 1 || manifest.items.len() != 24 {
        return Err("asset manifest must use schema 1 and contain exactly 24 items".into());
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
    if counts != [6, 4, 14] {
        return Err(
            "asset manifest must contain six characters, four backgrounds and fourteen props"
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
    fn all_twenty_four_native_projects_decode_evaluate_and_export_correct_alpha() {
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
