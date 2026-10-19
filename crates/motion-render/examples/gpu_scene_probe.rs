//! Explicit GPU scene/artwork experiment. Retained pixels and independent SVG
//! inputs are evidence, not an asserted CPU/GPU pixel-equivalence result.
use motion_render::{
    gpu::Backend,
    gpu_scene,
    scene_commands::{self, Labels},
};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Read,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

const MAX_RETAINED_ASSET_BYTES: usize = 64 * 1024 * 1024;
const MAX_DECODED_PIXELS: u64 = 4 * 1024 * 1024;
fn bounded(path: &Path, limit: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    if !file.metadata()?.is_file() {
        return Err("probe inputs must be regular local files".into());
    }
    let mut bytes = Vec::new();
    file.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > limit {
        return Err("probe input exceeds its byte budget".into());
    }
    Ok(bytes)
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

struct RetainedImage {
    id: String,
    bytes: Vec<u8>,
    path: String,
}
struct Inputs {
    source: String,
    project: Option<motion_project::ProjectDocument>,
    original: Vec<u8>,
}
fn load_inputs(path: &Path) -> Result<Inputs, Box<dyn std::error::Error>> {
    let project = path
        .extension()
        .is_some_and(|v| v.eq_ignore_ascii_case("kilsat"));
    let original = bounded(
        path,
        if project {
            motion_project::MAX_DOCUMENT_BYTES
        } else {
            8 * 1024 * 1024
        },
    )?;
    if project {
        let document = motion_project::decode(&original)?;
        Ok(Inputs {
            source: document.source.clone(),
            project: Some(document),
            original,
        })
    } else {
        let source = String::from_utf8(original.clone())?;
        Ok(Inputs {
            source,
            project: None,
            original,
        })
    }
}
fn samples(
    script: &motion_script::Script,
    selector: &str,
    seconds: f64,
) -> Result<Vec<(usize, f64, f64)>, Box<dyn std::error::Error>> {
    if selector != "sequence" {
        // Preserve the legacy script-only selector's explicit transition rejection.
        if script
            .scenes
            .iter()
            .any(|s| s.composition.transition != motion_script::Transition::Cut)
        {
            return Err(
                "use sequence instead of SCENE_INDEX to sample authored cuts/crossfades".into(),
            );
        }
        return Ok(vec![(selector.parse()?, seconds, 1.0)]);
    }
    let sample = (seconds * 24_000.0).round();
    if !seconds.is_finite() || seconds < 0.0 || sample >= u64::MAX as f64 {
        return Err("sequence seconds must round to a finite nonnegative sample".into());
    }
    let timeline = motion_scene::SequenceTimeline::compile(script)?;
    Ok(timeline
        .sample(sample as u64)?
        .iter()
        .map(|s| (s.scene_index, s.local_sample as f64 / 24_000.0, s.weight))
        .collect())
}
fn load_images(
    path: &Path,
    project: Option<&motion_project::ProjectDocument>,
    scenes: &[(&str, motion_scene::EvaluatedScene)],
) -> Result<
    (
        BTreeMap<String, motion_art::ImageResource>,
        Vec<RetainedImage>,
    ),
    Box<dyn std::error::Error>,
> {
    let Some(project) = project else {
        return Ok((BTreeMap::new(), Vec::new()));
    };
    let mut required = BTreeSet::new();
    for (id, scene) in scenes {
        for plane in [motion_art::Plane::Background, motion_art::Plane::Foreground] {
            for step in project.art.composition(id, plane, scene.time)? {
                if let motion_art::CompositionStep::Layer(layer) = step {
                    if layer
                        .attachment
                        .as_ref()
                        .is_some_and(|id| !scene.actors.iter().any(|a| &a.id == id))
                    {
                        continue;
                    }
                    if let motion_art::Content::Image { asset_id, .. } = &layer.content {
                        required.insert(asset_id.clone());
                    }
                }
            }
        }
    }
    let mut images = BTreeMap::new();
    let mut retained = Vec::new();
    let mut encoded = 0usize;
    let mut pixels = 0u64;
    for id in required {
        let asset = project
            .assets
            .iter()
            .find(|asset| asset.id == id && asset.kind == motion_project::AssetKind::Image)
            .ok_or("missing image asset reference")?;
        let resolved = motion_project::resolve_asset(path, asset)?;
        let remaining = MAX_RETAINED_ASSET_BYTES - encoded;
        let bytes = bounded(
            &resolved,
            remaining.min(motion_art::MAX_IMAGE_BYTES as usize),
        )?;
        encoded += bytes.len();
        let image = motion_art::decode_image(&bytes)?;
        pixels += u64::from(image.width()) * u64::from(image.height());
        if pixels > MAX_DECODED_PIXELS {
            return Err("probe decoded images exceed the GPU's combined 4 Mi pixel budget".into());
        }
        let retained_path = format!("assets/{}.image", retained.len());
        retained.push(RetainedImage {
            id: id.clone(),
            bytes,
            path: retained_path,
        });
        images.insert(id, image);
    }
    Ok((images, retained))
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 8 || args[1] != "--omit-labels-acknowledged" {
        return Err("usage: gpu_scene_probe --metal|--vulkan|--gl --omit-labels-acknowledged SCRIPT_OR_KILSAT SCENE_INDEX|sequence SECONDS WIDTH HEIGHT NEW_OUTPUT_DIRECTORY".into());
    }
    let backend = match args[0].as_str() {
        "--metal" => Backend::Metal,
        "--vulkan" => Backend::Vulkan,
        "--gl" => Backend::Gl,
        _ => return Err("explicit backend required".into()),
    };
    let source_path = PathBuf::from(&args[2]);
    let input = load_inputs(&source_path)?;
    let script = motion_script::parse(&input.source)?;
    script.validate()?;
    let seconds: f64 = args[4].parse()?;
    let width: u32 = args[5].parse()?;
    let height: u32 = args[6].parse()?;
    let output = PathBuf::from(&args[7]);
    if output.exists() {
        return Err("output already exists".into());
    }
    let contributions = samples(&script, &args[3], seconds)?;
    let mut scenes = Vec::new();
    for (index, local, _) in &contributions {
        let scene = motion_scene::evaluate(&script, *index, *local)?;
        scenes.push((script.scenes[*index].id.as_str(), scene));
    }
    let (images, retained) = load_images(&source_path, input.project.as_ref(), &scenes)?;
    let empty = motion_art::Document::default();
    let art = input.project.as_ref().map_or(&empty, |p| &p.art);
    let mut requests = Vec::new();
    let mut references = Vec::new();
    for (id, scene) in &scenes {
        requests.push(scene_commands::build_with_art(
            scene,
            width,
            height,
            &[],
            Labels::OmitAcknowledged,
            id,
            art,
            &images,
        )?);
        references.push(motion_render::render_svg_with_art(
            scene,
            width,
            height,
            &[],
            id,
            art,
            &images,
        )?);
    }
    let mut requests = requests.into_iter();
    let first = requests.next().ok_or("sequence produced no scene")?;
    let request = match requests.next() {
        Some(second) => scene_commands::crossfade(first, second, contributions[1].2)?,
        None => first,
    };
    let cancel = AtomicBool::new(false);
    let frame = gpu_scene::GpuSceneRenderer::new(backend, &cancel)?.render(&request, &cancel)?;
    // No output path exists until every input/reference/request and GPU readback succeeds.
    std::fs::create_dir(&output)?;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        std::fs::write(output.join("scene.rgba"), &frame.rgba)?;
        std::fs::write(output.join("source.yaml"), &input.source)?;
        let input_name = if input.project.is_some() {
            "input.kilsat"
        } else {
            "input.yaml"
        };
        std::fs::write(output.join(input_name), &input.original)?;
        let mut evidence = Vec::new();
        if !retained.is_empty() {
            std::fs::create_dir(output.join("assets"))?;
        }
        for asset in &retained {
            std::fs::write(output.join(&asset.path), &asset.bytes)?;
            evidence.push(serde_json::json!({"id":asset.id,"path":asset.path,"sha256":hash(&asset.bytes),"bytes":asset.bytes.len()}));
        }
        if let Some(project) = &input.project {
            let mut replay = project.clone();
            for asset in &retained {
                replay
                    .assets
                    .iter_mut()
                    .find(|a| a.id == asset.id)
                    .ok_or("replay asset missing")?
                    .path = asset.path.clone();
            }
            std::fs::write(
                output.join("replay.kilsat"),
                motion_project::encode(&replay)?,
            )?;
        }
        let mut reference_paths = Vec::new();
        for (index, svg) in references.iter().enumerate() {
            let name = if references.len() == 1 {
                "reference.svg".into()
            } else {
                format!("reference-{index}.svg")
            };
            std::fs::write(output.join(&name), svg)?;
            reference_paths.push(name);
        }
        std::fs::write(
            output.join("report.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema_version":2,"experiment":"evaluated-artwork-v2","adapter":frame.adapter_name,
                "backend":frame.backend,"device_type":frame.device_type,"width":frame.width,"height":frame.height,
                "selector":args[3],"seconds":seconds,"contributions":contributions,"format":"rgba8unorm-top-left",
                "row_padding_removed":true,"labels":"omitted-with-explicit-acknowledgement",
                "mouths":"rest-no-audio-preparation","artwork":if input.project.is_some(){"project-selected-sample"}else{"empty-script-only"},
                "input_file":input_name,"input_sha256":hash(&input.original),"retained_assets":evidence,
                "replay_scope":"selected sample only; unused image/audio assets are not copied",
                "cpu_references":reference_paths,"cpu_reference":"separate SVG-with-labels per contribution; use reported weights after rasterization",
                "pixel_parity":"not-measured","validation_errors":frame.validation_errors,"full_scene_parity":false,
                "implementation_commit":std::env::var("KILSAT_IMPLEMENTATION_COMMIT").ok(),
            }))?,
        )?;
        Ok(())
    })();
    if result.is_err() {
        let _ = std::fs::remove_dir_all(&output);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn project_probe_reads_exact_active_image_bytes_and_rejects_escaped_asset_refs() {
        use motion_art::*;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        struct Dir(PathBuf);
        impl Drop for Dir {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let directory = Dir(std::env::temp_dir().join(format!(
            "kilsat-gpu-project-probe-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        )));
        std::fs::create_dir(&directory.0).unwrap();
        let source = "characters: {}\nscenes: [{id: s, duration: 1, events: []}]\n";
        let mut project = motion_project::ProjectDocument::new(source).unwrap();
        project.assets.push(motion_project::AssetReference {
            id: "i".into(),
            path: "image.png".into(),
            kind: motion_project::AssetKind::Image,
        });
        project.art.layers.push(Layer {
            id: "image".into(),
            scene_id: "s".into(),
            name: "image".into(),
            visible: true,
            plane: Plane::Foreground,
            active: None,
            opacity: 1.0,
            attachment: None,
            clip: None,
            clip_shape: None,
            blend: BlendMode::Normal,
            content: Content::Image {
                asset_id: "i".into(),
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0,
                },
            },
        });
        let image = ImageResource::from_rgba(1, 1, vec![5, 10, 15, 128]).unwrap();
        std::fs::write(directory.0.join("image.png"), image.png()).unwrap();
        let path = directory.0.join("input.kilsat");
        let exact = motion_project::encode(&project).unwrap();
        std::fs::write(&path, &exact).unwrap();
        let loaded = load_inputs(&path).unwrap();
        assert_eq!(loaded.original, exact);
        assert_eq!(loaded.source, source);
        let script = motion_script::parse(source).unwrap();
        let scene = motion_scene::evaluate(&script, 0, 0.0).unwrap();
        let (decoded, retained) =
            load_images(&path, loaded.project.as_ref(), &[("s", scene)]).unwrap();
        assert_eq!(decoded["i"].rgba(), image.rgba());
        assert_eq!(retained[0].bytes, image.png());
        assert_eq!(retained[0].path, "assets/0.image");
        project.assets[0].path = "../outside.png".into();
        assert!(motion_project::encode(&project).is_err());
        assert!(bounded(&path, 1).is_err());
    }
    #[test]
    fn sequence_selector_uses_existing_sample_clock_and_transition_weights() {
        let script=motion_script::parse("characters: {}\nscenes: [{id: a, duration: 2, composition: {transition: {type: crossfade, duration: 1}}, events: []}, {id: b, duration: 2, events: []}]").unwrap();
        let result = samples(&script, "sequence", 1.5).unwrap();
        assert_eq!(result, vec![(0, 1.5, 0.5), (1, 0.5, 0.5)]);
        assert!(samples(&script, "0", 1.5).is_err());
        assert!(samples(&script, "sequence", f64::NAN).is_err());
        assert!(samples(&script, "sequence", 3.0).is_err());
    }
}
