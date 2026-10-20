//! Adapter-free commands for the experimental GPU scene compositor.
//! No font rasterizer is provided: actor labels require explicit acknowledgement.
//! The SVG string carries `<text>` labels, but the CPU raster path (resvg built
//! without its `text` feature, no font database) currently draws none of them, so
//! omission matches the CPU pixels; see docs/gpu-parity.md.
use motion_lipsync::Viseme;
use motion_scene::{
    EvaluatedScene, CHEST, HEAD, HIP, LEFT_FOOT, LEFT_HAND, RIGHT_FOOT, RIGHT_HAND,
};

pub const MAX_COMMANDS: usize = 4096;
/// Bound the analytic fragment shader's command visits, not merely allocation.
pub const MAX_FRAGMENT_WORK: u64 = 64 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Labels {
    #[default]
    Require,
    /// Explicit geometry-only experiment. The in-repo CPU rasterizer also draws no
    /// label text (docs/gpu-parity.md), but SVG consumers that do have fonts would.
    OmitAcknowledged,
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Scene(#[from] crate::Error),
    #[error(transparent)]
    Gpu(#[from] crate::gpu::Error),
    #[error("unsupported experimental GPU feature: {0}")]
    UnsupportedFeature(&'static str),
    #[error("experimental GPU geometry or fragment-work budget exceeded")]
    Budget,
}

/// Five vec4 values, serialized explicitly: no unsafe casts or host-layout assumptions.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Command(pub [[f32; 4]; 5]);

#[derive(Debug)]
pub struct SceneCommands {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) canvas: [f32; 4],
    pub(crate) background: [f32; 4],
    pub(crate) opacity: f32,
    pub(crate) commands: Vec<Command>,
    pub(crate) points: Vec<[f32; 2]>,
    pub(crate) pixels: Vec<u32>,
}
impl SceneCommands {
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    pub fn command_count(&self) -> usize {
        self.commands.len()
    }
    pub(crate) fn bytes(&self) -> Vec<u8> {
        self.commands
            .iter()
            .flat_map(|c| c.0.iter().flatten().flat_map(|v| v.to_le_bytes()))
            .collect()
    }
}

/// Only an evaluated actor scene is accepted. Artwork callers must use
/// `build_with_art`; sequence callers must explicitly compose complete requests.
pub fn build(
    scene: &EvaluatedScene,
    width: u32,
    height: u32,
    mouths: &[(&str, Viseme)],
    labels: Labels,
) -> Result<SceneCommands, Error> {
    crate::validate_scene(scene, width, height)?;
    crate::gpu::row_layout(width, height)?;
    if mouths.len() > crate::MAX_ACTORS {
        return Err(Error::Budget);
    }
    if !scene.actors.is_empty() && labels == Labels::Require {
        return Err(Error::UnsupportedFeature(
            "actor font labels; select OmitAcknowledged for geometry-only experiments",
        ));
    }
    // Reserve a conservative upper bound before expanding any actor geometry.
    let upper = scene.actors.iter().try_fold(0usize, |sum, actor| {
        sum.checked_add(if actor.rigid_parts.is_empty() {
            16
        } else {
            actor.rigid_parts.len()
        })
        .ok_or(Error::Budget)
    })?;
    let polygon_work: u64 = scene
        .actors
        .iter()
        .flat_map(|a| &a.rigid_parts)
        .map(|p| p.points.len() as u64)
        .sum();
    if upper > MAX_COMMANDS
        || u64::from(width) * u64::from(height) * (upper as u64 + polygon_work + 1)
            > MAX_FRAGMENT_WORK
    {
        return Err(Error::Budget);
    }
    let scale = (width as f64 / scene.width as f64).min(height as f64 / scene.height as f64);
    let ox = (width as f64 - scene.width as f64 * scale) * 0.5;
    let oy = (height as f64 - scene.height as f64 * scale) * 0.5;
    let byte =
        |i| u8::from_str_radix(&scene.background_color[i..i + 2], 16).unwrap() as f32 / 255.0;
    let mut result = SceneCommands {
        width,
        height,
        canvas: [
            ox as f32,
            oy as f32,
            (scene.width as f64 * scale) as f32,
            (scene.height as f64 * scale) as f32,
        ],
        background: [byte(1), byte(3), byte(5), 1.0],
        opacity: scene.opacity as f32,
        commands: Vec::with_capacity(upper),
        points: Vec::new(),
        pixels: Vec::new(),
    };
    let (cx, cy) = scene.camera.project(0.0, 0.0, scene.width, scene.height);
    for actor in &scene.actors {
        let factor = scale * scene.camera.zoom;
        let (sin, cos) = actor.rotation.sin_cos();
        let a = cos * actor.scale.x * factor;
        let b = sin * actor.scale.x * factor;
        let c = -sin * actor.scale.y * factor;
        let d = cos * actor.scale.y * factor;
        let tx = ox + scale * (cx + scene.camera.zoom * actor.x);
        let ty = oy + scale * (cy + scene.camera.zoom * actor.y);
        let det = a * d - b * c;
        let inv = [
            (d / det) as f32,
            (-b / det) as f32,
            (-c / det) as f32,
            (a / det) as f32,
        ];
        let origin = [
            ((-d * tx + c * ty) / det) as f32,
            ((b * tx - a * ty) / det) as f32,
            (0.75 / (actor.scale.x.min(actor.scale.y) * factor)) as f32,
            0.0,
        ];
        if !actor.rigid_parts.is_empty() {
            for part in &actor.rigid_parts {
                let vertices: Vec<motion_art::Point> = part
                    .points
                    .iter()
                    .map(|p| motion_art::Point { x: p.x, y: p.y })
                    .collect();
                let geometry = points(&mut result, &vertices)?;
                result.commands.push(Command([
                    inv,
                    origin,
                    geometry,
                    part.color.map(|v| f32::from(v) / 255.0),
                    [7.0, 0.0, 0.0, 0.0],
                ]));
            }
            continue;
        }
        let mut push = |kind: f32, geom: [f32; 4], color: [f32; 4], radius: f32| {
            result.commands.push(Command([
                inv,
                origin,
                geom,
                color,
                [kind, radius, 0.0, 0.0],
            ]));
        };
        let bone = [126.0 / 255.0, 185.0 / 255.0, 226.0 / 255.0, 1.0];
        let skin = [245.0 / 255.0, 200.0 / 255.0, 162.0 / 255.0, 1.0];
        let mouth = [125.0 / 255.0, 45.0 / 255.0, 57.0 / 255.0, 1.0];
        let black = [0.0, 0.0, 0.0, 1.0];
        let p = &actor.skeleton;
        for (a, b) in [
            (HIP, CHEST),
            (CHEST, HEAD),
            (CHEST, LEFT_HAND),
            (CHEST, RIGHT_HAND),
            (HIP, LEFT_FOOT),
            (HIP, RIGHT_FOOT),
        ] {
            push(
                2.0,
                [p[a].x as f32, p[a].y as f32, p[b].x as f32, p[b].y as f32],
                bone,
                2.5,
            );
        }
        let hx = p[HEAD].x as f32;
        let hy = p[HEAD].y as f32;
        push(0.0, [hx, hy, 25.0, 0.0], skin, 0.0);
        push(0.0, [hx - 8.0, hy - 4.0, 2.5, 0.0], black, 0.0);
        push(0.0, [hx + 8.0, hy - 4.0, 2.5, 0.0], black, 0.0);
        let viseme = mouths
            .iter()
            .find(|(id, _)| *id == actor.id)
            .map(|(_, v)| *v)
            .unwrap_or(Viseme::Rest);
        let (w, h) = match viseme {
            Viseme::A => (14.0, 14.0),
            Viseme::O | Viseme::U => (8.0, 11.0),
            Viseme::E | Viseme::I => (18.0, 6.0),
            Viseme::Rest | Viseme::Closed => (13.0, 2.0),
            _ => (12.0, 5.0),
        };
        push(
            1.0,
            [hx, hy + 10.0, w / 2.0, h / 2.0],
            mouth,
            2.0_f32.min(h / 2.0),
        );
        if actor.emotion == "happy" && matches!(viseme, Viseme::Rest | Viseme::Closed) {
            push(3.0, [hx - 8.0, hy + 9.0, hx, hy + 14.0], mouth, 1.0);
            push(3.0, [hx, hy + 14.0, hx + 8.0, hy + 9.0], mouth, 1.0);
        }
        if actor.emotion == "surprised" {
            push(3.0, [hx - 12.0, hy - 13.0, hx - 4.0, hy - 15.0], black, 0.5);
            push(3.0, [hx + 4.0, hy - 13.0, hx + 12.0, hy - 15.0], black, 0.5);
        }
    }
    debug_assert!(result.commands.len() <= upper);
    Ok(result)
}

/// Build genuine GPU artwork commands. The decoded source resources are uploaded,
/// never a rasterized final frame. Labels retain the explicit omission policy.
#[allow(clippy::too_many_arguments)]
pub fn build_with_art(
    scene: &EvaluatedScene,
    width: u32,
    height: u32,
    mouths: &[(&str, Viseme)],
    labels: Labels,
    scene_id: &str,
    document: &motion_art::Document,
    images: &std::collections::BTreeMap<String, motion_art::ImageResource>,
) -> Result<SceneCommands, Error> {
    document
        .validate()
        .map_err(|e| crate::Error::InvalidArtwork(e.to_string()))?;
    let mut result = build(scene, width, height, mouths, labels)?;
    let actors = std::mem::take(&mut result.commands);
    let mut resources = std::collections::BTreeMap::new();
    let mut work = command_work(&actors) + 1;
    append_art(
        &mut result,
        scene,
        scene_id,
        document,
        images,
        motion_art::Plane::Background,
        &mut resources,
        &mut work,
    )?;
    result.commands.extend(actors);
    append_art(
        &mut result,
        scene,
        scene_id,
        document,
        images,
        motion_art::Plane::Foreground,
        &mut resources,
        &mut work,
    )?;
    if result.commands.len() > MAX_COMMANDS
        || u64::from(width) * u64::from(height) * work > MAX_FRAGMENT_WORK
    {
        return Err(Error::Budget);
    }
    Ok(result)
}

/// Compose already validated complete scene requests using the sequence evaluator's
/// second contribution weight. Both scenes remain real GPU commands, including
/// their own camera, artwork, masks, fades and opaque letterboxing.
pub fn crossfade(
    mut first: SceneCommands,
    second: SceneCommands,
    second_weight: f64,
) -> Result<SceneCommands, Error> {
    if first.dimensions() != second.dimensions()
        || !second_weight.is_finite()
        || !(0.0..=1.0).contains(&second_weight)
    {
        return Err(crate::Error::InvalidArtwork(
            "GPU crossfade requires equal dimensions and finite weight 0..1".into(),
        )
        .into());
    }
    if first
        .commands
        .iter()
        .chain(&second.commands)
        .any(|c| c.0[4][0] == 14.0)
    {
        return Err(Error::UnsupportedFeature(
            "nested complete-frame GPU transitions",
        ));
    }
    if first.points.len() + second.points.len() > MAX_GPU_POINTS
        || first.pixels.len() + second.pixels.len() > MAX_GPU_IMAGE_PIXELS
        || first.commands.len() + second.commands.len() + 1 > MAX_COMMANDS
    {
        return Err(Error::Budget);
    }
    let work = command_work(&first.commands) + command_work(&second.commands) + 3;
    if u64::from(first.width) * u64::from(first.height) * work > MAX_FRAGMENT_WORK {
        return Err(Error::Budget);
    }
    let point_offset = first.points.len() as f32;
    let pixel_offset = first.pixels.len() as f32;
    first.commands.push(Command([
        [0.0; 4],
        [second.opacity, second_weight as f32, 0.0, 0.0],
        second.canvas,
        second.background,
        [14.0, 0.0, 0.0, 0.0],
    ]));
    for mut command in second.commands {
        let kind = command.0[4][0];
        if kind == 5.0 || kind == 18.0 {
            command.0[4][1] += pixel_offset;
        }
        if kind == 6.0 || kind == 7.0 || kind == 11.0 || kind == 17.0 {
            command.0[2][0] += point_offset;
        }
        first.commands.push(command);
    }
    first.points.extend(second.points);
    first.pixels.extend(second.pixels);
    Ok(first)
}
fn command_work(commands: &[Command]) -> u64 {
    commands
        .iter()
        .map(|c| {
            let kind = c.0[4][0];
            if kind == 17.0 {
                c.0[2][3] as u64
            } else if kind == 6.0 || kind == 7.0 || kind == 11.0 || kind == 19.0 {
                c.0[2][1] as u64 + 1
            } else if kind == 5.0 || kind == 18.0 {
                4
            } else {
                1
            }
        })
        .sum()
}

/// Share decoded source storage between image content and alpha/luminance masks.
fn image_resource(
    result: &mut SceneCommands,
    images: &std::collections::BTreeMap<String, motion_art::ImageResource>,
    resources: &mut std::collections::BTreeMap<String, usize>,
    asset_id: &str,
) -> Result<(usize, u32, u32), Error> {
    let image = images
        .get(asset_id)
        .ok_or_else(|| crate::Error::InvalidArtwork(format!("missing decoded image {asset_id}")))?;
    let offset = if let Some(offset) = resources.get(asset_id) {
        *offset
    } else {
        let offset = result.pixels.len();
        let count = image.rgba().len() / 4;
        if offset
            .checked_add(count)
            .is_none_or(|n| n > MAX_GPU_IMAGE_PIXELS)
        {
            return Err(Error::Budget);
        }
        let pixels = image.rgba().as_chunks::<4>().0;
        result
            .pixels
            .extend(pixels.iter().copied().map(u32::from_le_bytes));
        resources.insert(asset_id.to_owned(), offset);
        offset
    };
    Ok((offset, image.width(), image.height()))
}

const MAX_GPU_IMAGE_PIXELS: usize = 4 * 1024 * 1024;
const MAX_GPU_POINTS: usize = 65_536;
pub(crate) const MAX_COMPOSITION_DEPTH: usize = 16;

fn art_error(error: motion_art::Error) -> Error {
    crate::Error::InvalidArtwork(error.to_string()).into()
}
fn rect(r: motion_art::Rect) -> [f32; 4] {
    [r.x as f32, r.y as f32, r.width as f32, r.height as f32]
}
fn color(c: motion_art::Color) -> [f32; 4] {
    [c.r, c.g, c.b, c.a].map(|v| f32::from(v) / 255.0)
}
fn blend(mode: motion_art::BlendMode) -> f32 {
    match mode {
        motion_art::BlendMode::Normal => 0.0,
        motion_art::BlendMode::Multiply => 1.0,
        motion_art::BlendMode::Screen => 2.0,
    }
}
fn points(result: &mut SceneCommands, values: &[motion_art::Point]) -> Result<[f32; 4], Error> {
    let offset = result.points.len();
    if offset
        .checked_add(values.len())
        .is_none_or(|n| n > MAX_GPU_POINTS)
    {
        return Err(Error::Budget);
    }
    result
        .points
        .extend(values.iter().map(|p| [p.x as f32, p.y as f32]));
    Ok([offset as f32, values.len() as f32, 0.0, 0.0])
}
#[allow(clippy::too_many_arguments)]
fn append_art(
    result: &mut SceneCommands,
    scene: &EvaluatedScene,
    scene_id: &str,
    document: &motion_art::Document,
    images: &std::collections::BTreeMap<String, motion_art::ImageResource>,
    plane: motion_art::Plane,
    resources: &mut std::collections::BTreeMap<String, usize>,
    work: &mut u64,
) -> Result<(), Error> {
    use motion_art::{ClipShape, CompositionStep, Content};
    let factor = result.canvas[2] as f64 / scene.width as f64 * scene.camera.zoom;
    let scale = result.canvas[2] as f64 / scene.width as f64;
    let (cx, cy) = scene.camera.project(0.0, 0.0, scene.width, scene.height);
    let mut depth = 0usize;
    for step in document
        .composition(scene_id, plane, scene.time)
        .map_err(art_error)?
    {
        match step {
            CompositionStep::EnterGroup(group) => {
                depth += 1;
                if depth >= MAX_COMPOSITION_DEPTH {
                    return Err(Error::Budget);
                }
                result.commands.push(Command([
                    [0.0; 4],
                    [0.0; 4],
                    [group.opacity as f32, blend(group.blend), 0.0, 0.0],
                    [0.0; 4],
                    [8.0, 0.0, 0.0, 0.0],
                ]));
                *work += 1;
            }
            CompositionStep::LeaveGroup => {
                depth -= 1;
                result.commands.push(Command([
                    [0.0; 4],
                    [0.0; 4],
                    [0.0; 4],
                    [0.0; 4],
                    [12.0, 0.0, 0.0, 0.0],
                ]));
                *work += 1;
            }
            CompositionStep::Layer(layer) => {
                if depth + 1 >= MAX_COMPOSITION_DEPTH {
                    return Err(Error::Budget);
                }
                let (x, y) = match &layer.attachment {
                    Some(id) => match scene.actors.iter().find(|a| &a.id == id) {
                        Some(actor) => (actor.x, actor.y),
                        None => continue,
                    },
                    None => (0.0, 0.0),
                };
                let inv = [(1.0 / factor) as f32, 0.0, 0.0, (1.0 / factor) as f32];
                let origin = [
                    (-(result.canvas[0] as f64 + scale * cx) / factor - x) as f32,
                    (-(result.canvas[1] as f64 + scale * cy) / factor - y) as f32,
                    (0.75 / factor) as f32,
                    0.0,
                ];
                result.commands.push(Command([
                    inv,
                    origin,
                    [layer.opacity as f32, blend(layer.blend), 0.0, 0.0],
                    [0.0; 4],
                    [8.0, 0.0, 0.0, 0.0],
                ]));
                if let Some(r) = layer.clip {
                    result.commands.push(Command([
                        inv,
                        origin,
                        rect(r),
                        [0.0; 4],
                        [9.0, 0.0, 0.0, 0.0],
                    ]));
                    *work += 1;
                }
                if let Some(mask) = &layer.clip_shape {
                    if mask.key_count() > 0
                        && matches!(mask, ClipShape::Polygon { .. } | ClipShape::Painted { .. })
                    {
                        return Err(Error::UnsupportedFeature(
                            "animated polygon or painted mask",
                        ));
                    }
                    match mask {
                        ClipShape::Painted {
                            base_visible,
                            strokes,
                            ..
                        } => {
                            result.commands.push(Command([
                                inv,
                                origin,
                                [f32::from(u8::from(*base_visible)), 0.0, 0.0, 0.0],
                                [0.0; 4],
                                [15.0, 0.0, 0.0, 0.0],
                            ]));
                            *work += 1;
                            for stroke in strokes {
                                let offset = result.points.len();
                                let mut count = 0usize;
                                let mut cost = 1u64;
                                for (coverage, radius_scale) in stroke.coverage_bands() {
                                    for primitive in stroke.band_geometry(radius_scale) {
                                        let record = match primitive {
                                            motion_art::MaskBrushPrimitive::Circle {
                                                center,
                                                radius,
                                            } => {
                                                cost += 2;
                                                [
                                                    [0.0, coverage as f32],
                                                    [center.x as f32, center.y as f32],
                                                    [radius as f32, 0.0],
                                                    [0.0; 2],
                                                    [0.0; 2],
                                                ]
                                            }
                                            motion_art::MaskBrushPrimitive::Polygon {
                                                points: vertices,
                                            } => {
                                                cost += 5;
                                                [
                                                    [1.0, coverage as f32],
                                                    [vertices[0].x as f32, vertices[0].y as f32],
                                                    [vertices[1].x as f32, vertices[1].y as f32],
                                                    [vertices[2].x as f32, vertices[2].y as f32],
                                                    [vertices[3].x as f32, vertices[3].y as f32],
                                                ]
                                            }
                                        };
                                        if result.points.len() + record.len() > MAX_GPU_POINTS
                                            || u64::from(result.width)
                                                * u64::from(result.height)
                                                * (*work + cost)
                                                > MAX_FRAGMENT_WORK
                                        {
                                            return Err(Error::Budget);
                                        }
                                        result.points.extend(record);
                                        count += 1;
                                    }
                                }
                                result.commands.push(Command([
                                    inv,
                                    origin,
                                    [
                                        offset as f32,
                                        count as f32,
                                        f32::from(u8::from(!stroke.erase)),
                                        cost as f32,
                                    ],
                                    [0.0; 4],
                                    [17.0, 0.0, 0.0, 0.0],
                                ]));
                                *work += cost;
                            }
                        }
                        ClipShape::Image {
                            asset_id, channel, ..
                        } => {
                            let (offset, width, height) =
                                image_resource(result, images, resources, asset_id)?;
                            result.commands.push(Command([
                                inv,
                                origin,
                                rect(mask.sample_image_rect(scene.time).ok_or(Error::Budget)?),
                                [
                                    f32::from(u8::from(
                                        *channel == motion_art::MaskChannel::Luminance,
                                    )),
                                    0.0,
                                    0.0,
                                    0.0,
                                ],
                                [18.0, offset as f32, width as f32, height as f32],
                            ]));
                            *work += 4;
                        }
                        ClipShape::Polygon {
                            points: vertices,
                            feather,
                            ..
                        } if *feather > 0.0 => {
                            let geometry = points(result, vertices)?;
                            result.commands.push(Command([
                                inv,
                                origin,
                                geometry,
                                [0.0; 4],
                                [19.0, *feather as f32, 0.0, 0.0],
                            ]));
                            // Full-canvas erf evaluation per vertex edge, like the hard polygon.
                            *work += vertices.len() as u64;
                        }
                        ClipShape::Polygon {
                            points: vertices, ..
                        } => {
                            let geometry = points(result, vertices)?;
                            result.commands.push(Command([
                                inv,
                                origin,
                                geometry,
                                [0.0; 4],
                                [11.0, 0.0, 0.0, 0.0],
                            ]));
                            *work += vertices.len() as u64;
                        }
                        _ => {
                            let (r, feather) =
                                mask.sample_ellipse(scene.time).ok_or(Error::Budget)?;
                            result.commands.push(Command([
                                inv,
                                origin,
                                rect(r),
                                [0.0; 4],
                                [10.0, feather as f32, 0.0, 0.0],
                            ]));
                            *work += 1;
                        }
                    }
                }
                let (kind, geometry, paint, parameters, cost) = match &layer.content {
                    Content::Rectangle { rect: r, color: c } => (
                        1.0,
                        [
                            (r.x + r.width / 2.0) as f32,
                            (r.y + r.height / 2.0) as f32,
                            (r.width / 2.0) as f32,
                            (r.height / 2.0) as f32,
                        ],
                        color(*c),
                        [1.0, 0.0, 0.0, 0.0],
                        1,
                    ),
                    Content::Ellipse { rect: r, color: c } => {
                        (4.0, rect(*r), color(*c), [4.0, 0.0, 0.0, 0.0], 1)
                    }
                    Content::Stroke {
                        points: vertices,
                        color: c,
                        width,
                    } => {
                        let mut geometry = points(result, vertices)?;
                        geometry[2] = (*width / 2.0) as f32;
                        (
                            6.0,
                            geometry,
                            color(*c),
                            [6.0, 0.0, 0.0, 0.0],
                            vertices.len() as u64,
                        )
                    }
                    Content::Image { asset_id, rect: r } => {
                        let (offset, width, height) =
                            image_resource(result, images, resources, asset_id)?;
                        (
                            5.0,
                            rect(*r),
                            [1.0; 4],
                            [5.0, offset as f32, width as f32, height as f32],
                            4,
                        )
                    }
                };
                debug_assert_eq!(kind, parameters[0]);
                result
                    .commands
                    .push(Command([inv, origin, geometry, paint, parameters]));
                result.commands.push(Command([
                    [0.0; 4],
                    [0.0; 4],
                    [0.0; 4],
                    [0.0; 4],
                    [12.0, 0.0, 0.0, 0.0],
                ]));
                *work += cost + 2;
            }
        }
        if result.commands.len() > MAX_COMMANDS
            || u64::from(result.width) * u64::from(result.height) * *work > MAX_FRAGMENT_WORK
        {
            return Err(Error::Budget);
        }
    }
    Ok(())
}

pub fn reject_artwork_or_sequence() -> Error {
    Error::UnsupportedFeature(
        "legacy geometry-only entry point does not load artwork or sample sequence transitions; use build_with_art/crossfade explicitly",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn scene() -> EvaluatedScene {
        crate::tests::scene()
    }
    fn artwork_layer(id: &str, content: motion_art::Content) -> motion_art::Layer {
        motion_art::Layer {
            id: id.into(),
            scene_id: "s".into(),
            name: id.into(),
            visible: true,
            plane: motion_art::Plane::Foreground,
            active: None,
            opacity: 0.5,
            attachment: None,
            clip: None,
            clip_shape: None,
            blend: motion_art::BlendMode::Normal,
            content,
        }
    }
    #[test]
    fn manual_pressure_fixture_fits_documented_probe_budget_before_adapter_creation() {
        let document = motion_project::decode(include_bytes!(
            "../../../examples/painted-mask-input.kilsat"
        ))
        .unwrap();
        let script = motion_script::parse(&document.source).unwrap();
        let evaluated = motion_scene::evaluate(&script, 0, 1.0).unwrap();
        let commands = build_with_art(
            &evaluated,
            256,
            144,
            &[],
            Labels::OmitAcknowledged,
            &script.scenes[0].id,
            &document.art,
            &std::collections::BTreeMap::new(),
        )
        .unwrap();
        assert!(commands
            .commands
            .iter()
            .any(|command| command.0[4][0] == 17.0));
    }
    #[test]
    fn painted_masks_compile_with_explicit_base_before_adapter_creation() {
        use motion_art::*;
        let mut layer = artwork_layer(
            "painted",
            Content::Rectangle {
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 20.0,
                    height: 20.0,
                },
                color: Color {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 255,
                },
            },
        );
        layer.clip_shape = Some(ClipShape::Painted {
            motion: Vec::new(),
            base_visible: true,
            strokes: vec![],
        });
        let document = Document {
            layers: vec![layer],
            ..Document::default()
        };
        let commands = build_with_art(
            &scene(),
            64,
            64,
            &[],
            Labels::OmitAcknowledged,
            "s",
            &document,
            &std::collections::BTreeMap::new(),
        )
        .unwrap();
        let mask = commands
            .commands
            .iter()
            .find(|c| c.0[4][0] == 15.0)
            .unwrap();
        assert_eq!(mask.0[2][0], 1.0);
    }
    #[test]
    fn painted_pressure_softness_order_and_crossfade_relocation_are_real_commands() {
        use motion_art::*;
        let mut layer = artwork_layer(
            "mask",
            Content::Rectangle {
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 20.0,
                    height: 20.0,
                },
                color: Color {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 255,
                },
            },
        );
        let stroke = MaskStroke {
            points: vec![Point { x: 2.0, y: 3.0 }, Point { x: 12.0, y: 3.0 }],
            radius: 4.0,
            erase: false,
            pressures: vec![0.25, 1.0],
            hardness: 0.5,
        };
        let mut erase = stroke.clone();
        erase.erase = true;
        erase.hardness = 1.0;
        layer.clip_shape = Some(ClipShape::Painted {
            motion: Vec::new(),
            base_visible: false,
            strokes: vec![stroke.clone(), erase, stroke],
        });
        let document = Document {
            layers: vec![layer],
            ..Default::default()
        };
        let make = || {
            build_with_art(
                &scene(),
                64,
                64,
                &[],
                Labels::OmitAcknowledged,
                "s",
                &document,
                &std::collections::BTreeMap::new(),
            )
            .unwrap()
        };
        let commands = make();
        let strokes: Vec<_> = commands
            .commands
            .iter()
            .filter(|c| c.0[4][0] == 17.0)
            .collect();
        assert_eq!(strokes.len(), 3);
        assert_eq!(
            strokes.iter().map(|c| c.0[2][2]).collect::<Vec<_>>(),
            vec![1.0, 0.0, 1.0]
        );
        assert_eq!(strokes[0].0[2][1], 96.0);
        let base = commands
            .commands
            .iter()
            .find(|c| c.0[4][0] == 15.0)
            .unwrap();
        assert_eq!(base.0[2][0], 0.0);
        // Two endpoint disks have exactly the pressure ratio, followed by tangent quad.
        assert_eq!(commands.points[2][0] * 4.0, commands.points[7][0]);
        let offset = commands.points.len() as f32;
        let count = commands.commands.len();
        let mixed = crossfade(commands, make(), 0.5).unwrap();
        let first = mixed.commands[..count]
            .iter()
            .find(|c| c.0[4][0] == 17.0)
            .unwrap();
        let second = mixed.commands[count + 1..]
            .iter()
            .find(|c| c.0[4][0] == 17.0)
            .unwrap();
        assert_eq!(second.0[2][0], first.0[2][0] + offset);
        assert!(mixed.pixels.is_empty());
    }
    #[test]
    fn painted_fragment_work_is_rejected_before_any_adapter() {
        use motion_art::*;
        let mut layer = artwork_layer(
            "mask",
            Content::Rectangle {
                rect: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 20.0,
                    height: 20.0,
                },
                color: Color {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 255,
                },
            },
        );
        layer.clip_shape = Some(ClipShape::Painted {
            motion: Vec::new(),
            base_visible: false,
            strokes: vec![MaskStroke {
                points: vec![Point { x: 10.0, y: 10.0 }],
                radius: 4.0,
                erase: false,
                pressures: vec![0.5],
                hardness: 0.5,
            }],
        });
        let mut empty = scene();
        empty.actors.clear();
        let document = Document {
            layers: vec![layer],
            ..Default::default()
        };
        assert!(matches!(
            build_with_art(
                &empty,
                2048,
                2048,
                &[],
                Labels::Require,
                "s",
                &document,
                &std::collections::BTreeMap::new()
            ),
            Err(super::Error::Budget)
        ));
    }
    #[test]
    fn image_masks_share_sources_encode_channels_and_relocate_crossfades() {
        use motion_art::*;
        let bounds = Rect {
            x: 10.0,
            y: 20.0,
            width: 30.0,
            height: 40.0,
        };
        let mut layer = artwork_layer(
            "masked",
            Content::Image {
                asset_id: "i".into(),
                rect: bounds,
            },
        );
        layer.clip_shape = Some(ClipShape::Image {
            asset_id: "i".into(),
            rect: bounds,
            channel: MaskChannel::Luminance,
            keys: Vec::new(),
        });
        let mut document = Document {
            layers: vec![layer],
            ..Default::default()
        };
        let images = std::collections::BTreeMap::from([(
            "i".into(),
            ImageResource::from_rgba(1, 1, vec![255, 0, 0, 128]).unwrap(),
        )]);
        let make = |document: &Document, images: &std::collections::BTreeMap<_, _>| {
            build_with_art(
                &scene(),
                64,
                64,
                &[],
                Labels::OmitAcknowledged,
                "s",
                document,
                images,
            )
        };
        let commands = make(&document, &images).unwrap();
        assert_eq!(commands.pixels.len(), 1);
        let mask = commands
            .commands
            .iter()
            .find(|c| c.0[4][0] == 18.0)
            .unwrap();
        assert_eq!(mask.0[2], rect(bounds));
        assert_eq!(mask.0[3][0], 1.0);
        assert_eq!(mask.0[4], [18.0, 0.0, 1.0, 1.0]);
        let mixed = crossfade(commands, make(&document, &images).unwrap(), 0.5).unwrap();
        let masks: Vec<_> = mixed
            .commands
            .iter()
            .filter(|c| c.0[4][0] == 18.0)
            .collect();
        assert_eq!(masks[1].0[4][1], 1.0);
        assert!(make(&document, &std::collections::BTreeMap::new()).is_err());
        if let Some(ClipShape::Image { channel, .. }) = &mut document.layers[0].clip_shape {
            *channel = MaskChannel::Alpha;
        }
        let alpha = make(&document, &images).unwrap();
        let alpha_mask = alpha.commands.iter().find(|c| c.0[4][0] == 18.0).unwrap();
        assert_eq!(alpha_mask.0[3][0], 0.0);
        let huge = std::collections::BTreeMap::from([(
            "i".into(),
            ImageResource::from_rgba(2049, 2048, vec![0; 2049 * 2048 * 4]).unwrap(),
        )]);
        assert!(matches!(make(&document, &huge), Err(super::Error::Budget)));
        if let Some(ClipShape::Image { keys, .. }) = &mut document.layers[0].clip_shape {
            *keys = vec![
                RectMaskKey {
                    time: 0.0,
                    rect: bounds,
                },
                RectMaskKey {
                    time: 2.0,
                    rect: Rect { x: 30.0, ..bounds },
                },
            ];
        }
        let sample = |time| {
            let mut evaluated = scene();
            evaluated.time = time;
            let commands = build_with_art(
                &evaluated,
                64,
                64,
                &[],
                Labels::OmitAcknowledged,
                "s",
                &document,
                &images,
            )
            .unwrap();
            commands
                .commands
                .into_iter()
                .find(|c| c.0[4][0] == 18.0)
                .unwrap()
        };
        let middle = sample(1.0);
        assert_eq!(middle.0[2][0], 20.0);
        assert_eq!(sample(2.0).0[2][0], 30.0);
        assert_eq!(middle, sample(1.0));
        assert_eq!(sample(0.0).0[2][0], 10.0);
    }
    #[test]
    fn artwork_image_deduplication_masks_groups_and_order_are_real_commands() {
        use motion_art::*;
        let r = Rect {
            x: 10.0,
            y: 20.0,
            width: 30.0,
            height: 40.0,
        };
        let mut a = artwork_layer(
            "a",
            Content::Image {
                asset_id: "i".into(),
                rect: r,
            },
        );
        a.plane = Plane::Background;
        a.clip_shape = Some(ClipShape::SoftEllipse {
            rect: r,
            feather: 0.25,
            keys: vec![],
        });
        let mut b = artwork_layer(
            "b",
            Content::Image {
                asset_id: "i".into(),
                rect: r,
            },
        );
        b.clip = Some(r);
        let document = Document {
            layers: vec![a, b],
            ..Default::default()
        };
        let images = std::collections::BTreeMap::from([(
            "i".into(),
            ImageResource::from_rgba(1, 1, vec![10, 20, 30, 128]).unwrap(),
        )]);
        let c = build_with_art(
            &scene(),
            64,
            64,
            &[],
            Labels::OmitAcknowledged,
            "s",
            &document,
            &images,
        )
        .unwrap();
        assert_eq!(c.pixels, vec![u32::from_le_bytes([10, 20, 30, 128])]);
        let kinds: Vec<_> = c.commands.iter().map(|c| c.0[4][0]).collect();
        assert_eq!(&kinds[..4], &[8.0, 10.0, 5.0, 12.0]);
        assert_eq!(&kinds[kinds.len() - 4..], &[8.0, 9.0, 5.0, 12.0]);
        assert_eq!(c.commands[1].0[4][1], 0.25);
        assert!(build_with_art(
            &scene(),
            64,
            64,
            &[],
            Labels::OmitAcknowledged,
            "s",
            &document,
            &std::collections::BTreeMap::new()
        )
        .is_err());
    }
    #[test]
    fn isolated_groups_and_polygon_strokes_preserve_coverage_not_segment_overdraw() {
        use motion_art::*;
        let vertices = vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 20.0, y: 0.0 },
            Point { x: 10.0, y: 20.0 },
        ];
        let mut layer = artwork_layer(
            "a",
            Content::Stroke {
                points: vertices.clone(),
                color: Color {
                    r: 255,
                    g: 0,
                    b: 0,
                    a: 128,
                },
                width: 4.0,
            },
        );
        layer.clip_shape = Some(ClipShape::Polygon {
            motion: Vec::new(),
            points: vertices,
            feather: 0.0,
        });
        let document = Document {
            layers: vec![layer],
            groups: vec![Group {
                id: "g".into(),
                scene_id: "s".into(),
                plane: Plane::Foreground,
                name: "g".into(),
                visible: true,
                opacity: 0.5,
                blend: BlendMode::Multiply,
                children: vec![ArtUnit::Layer("a".into())],
            }],
            order: vec![ArtUnit::Group("g".into())],
        };
        let c = build_with_art(
            &scene(),
            64,
            64,
            &[],
            Labels::OmitAcknowledged,
            "s",
            &document,
            &std::collections::BTreeMap::new(),
        )
        .unwrap();
        let tail = &c.commands[c.commands.len() - 6..];
        assert_eq!(
            tail.iter().map(|c| c.0[4][0]).collect::<Vec<_>>(),
            vec![8.0, 8.0, 11.0, 6.0, 12.0, 12.0]
        );
        assert_eq!(tail[0].0[2], [0.5, 1.0, 0.0, 0.0]);
        assert_eq!(c.points.len(), 6);
        assert_eq!(tail[3].0[2], [3.0, 3.0, 2.0, 0.0]);
    }
    #[test]
    fn complete_frame_crossfade_relocates_source_resources_and_rejects_invalid_weights() {
        use motion_art::*;
        let r = Rect {
            x: 0.0,
            y: 0.0,
            width: 20.0,
            height: 20.0,
        };
        let document = Document {
            layers: vec![artwork_layer(
                "a",
                Content::Image {
                    asset_id: "i".into(),
                    rect: r,
                },
            )],
            ..Default::default()
        };
        let images = std::collections::BTreeMap::from([(
            "i".into(),
            ImageResource::from_rgba(1, 1, vec![1, 2, 3, 255]).unwrap(),
        )]);
        let make = || {
            build_with_art(
                &scene(),
                64,
                64,
                &[],
                Labels::OmitAcknowledged,
                "s",
                &document,
                &images,
            )
            .unwrap()
        };
        let n = make().command_count();
        let mixed = crossfade(make(), make(), 0.25).unwrap();
        assert_eq!(mixed.command_count(), n * 2 + 1);
        assert_eq!(mixed.pixels.len(), 2);
        assert_eq!(mixed.commands[n].0[4][0], 14.0);
        assert_eq!(mixed.commands[n].0[1][1], 0.25);
        assert_eq!(mixed.commands.last().unwrap().0[4][0], 12.0);
        assert_eq!(mixed.commands[mixed.commands.len() - 2].0[4][1], 1.0);
        assert!(crossfade(make(), make(), f64::NAN).is_err());
        assert!(crossfade(mixed, make(), 0.5).is_err());
        let other = build(&scene(), 65, 64, &[], Labels::OmitAcknowledged).unwrap();
        assert!(crossfade(make(), other, 0.5).is_err());
    }
    #[test]
    fn oversized_gpu_source_image_is_rejected_before_adapter_initialization() {
        use motion_art::*;
        let image = ImageResource::from_rgba(2049, 2048, vec![0; 2049 * 2048 * 4]).unwrap();
        let images = std::collections::BTreeMap::from([("large".into(), image)]);
        let document = Document {
            layers: vec![artwork_layer(
                "a",
                Content::Image {
                    asset_id: "large".into(),
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 10.0,
                        height: 10.0,
                    },
                },
            )],
            ..Default::default()
        };
        assert!(matches!(
            build_with_art(
                &scene(),
                64,
                64,
                &[],
                Labels::OmitAcknowledged,
                "s",
                &document,
                &images
            ),
            Err(super::Error::Budget)
        ));
    }
    #[test]
    fn animated_polygon_and_painted_masks_are_reported_unsupported_not_drawn_static() {
        use motion_art::*;
        let points = vec![
            Point { x: 0.0, y: 0.0 },
            Point { x: 20.0, y: 0.0 },
            Point { x: 10.0, y: 20.0 },
        ];
        let motion = vec![MaskMotionKey::default()];
        for shape in [
            ClipShape::Polygon {
                points: points.clone(),
                feather: 0.0,
                motion: motion.clone(),
            },
            ClipShape::Painted {
                base_visible: true,
                strokes: Vec::new(),
                motion: motion.clone(),
            },
        ] {
            let mut layer = artwork_layer(
                "a",
                Content::Rectangle {
                    rect: Rect {
                        x: 0.0,
                        y: 0.0,
                        width: 10.0,
                        height: 10.0,
                    },
                    color: Color {
                        r: 1,
                        g: 2,
                        b: 3,
                        a: 255,
                    },
                },
            );
            layer.clip_shape = Some(shape);
            let document = Document {
                layers: vec![layer],
                ..Default::default()
            };
            assert!(matches!(
                build_with_art(
                    &scene(),
                    64,
                    64,
                    &[],
                    Labels::OmitAcknowledged,
                    "s",
                    &document,
                    &std::collections::BTreeMap::new()
                ),
                Err(super::Error::UnsupportedFeature(_))
            ));
        }
    }
    #[test]
    fn labels_cannot_silently_disappear() {
        assert!(matches!(
            build(&scene(), 640, 360, &[], Labels::Require),
            Err(Error::UnsupportedFeature(_))
        ));
    }
    #[test]
    fn camera_root_and_letterbox_are_shared_scene_transforms() {
        let mut s = scene();
        s.camera.x = 30.0;
        s.camera.y = -10.0;
        s.camera.zoom = 1.5;
        s.actors[0].rotation = 0.37;
        s.actors[0].scale = motion_scene::Vec2::new(1.3, 0.7);
        let commands = build(&s, 640, 480, &[], Labels::OmitAcknowledged).unwrap();
        assert_eq!(commands.canvas, [0.0, 60.0, 640.0, 360.0]);
        let world = s.actors[0].local_to_world(s.actors[0].skeleton[HEAD]);
        let (x, y) = s.camera.project(world.x, world.y, s.width, s.height);
        let c = &commands.commands[6].0;
        let local_x = c[0][0] * x as f32 + c[0][2] * (y + 60.0) as f32 + c[1][0];
        let local_y = c[0][1] * x as f32 + c[0][3] * (y + 60.0) as f32 + c[1][1];
        assert!((local_x - c[2][0]).abs() < 0.001);
        assert!((local_y - c[2][1]).abs() < 0.001);
    }
    #[test]
    fn scalar_validation_and_fragment_work_precede_adapter_creation() {
        let mut s = scene();
        s.opacity = f64::NAN;
        assert!(matches!(
            build(&s, 640, 360, &[], Labels::OmitAcknowledged),
            Err(Error::Scene(_))
        ));
        let s = scene();
        assert!(matches!(
            build(&s, 2048, 2048, &[], Labels::OmitAcknowledged),
            Err(Error::Budget)
        ));
        let mouths = vec![("EMMA", Viseme::Rest); crate::MAX_ACTORS + 1];
        assert!(matches!(
            build(&s, 64, 64, &mouths, Labels::OmitAcknowledged),
            Err(Error::Budget)
        ));
    }
    #[test]
    fn empty_scene_background_opacity_and_serialization() {
        let mut s = scene();
        s.actors.clear();
        s.opacity = 0.25;
        let c = build(&s, 65, 64, &[], Labels::Require).unwrap();
        assert_eq!(c.command_count(), 0);
        assert_eq!(c.dimensions(), (65, 64));
        assert_eq!(c.opacity, 0.25);
        assert!(c.bytes().is_empty());
        let c = build(&scene(), 640, 360, &[], Labels::OmitAcknowledged).unwrap();
        assert_eq!(c.bytes().len(), c.command_count() * 80);
    }
    #[test]
    fn authored_skeleton_offsets_reach_gpu_commands_without_reevaluation() {
        let mut s = scene();
        s.actors[0].skeleton[RIGHT_HAND] = motion_scene::Vec2::new(99.0, -77.0);
        let c = build(&s, 640, 360, &[], Labels::OmitAcknowledged).unwrap();
        assert_eq!(c.commands[3].0[2][2..], [99.0, -77.0]);
        s.opacity = 0.375;
        let c = build(&s, 640, 360, &[], Labels::OmitAcknowledged).unwrap();
        assert_eq!(c.opacity, 0.375);
    }
    #[test]
    fn mouth_dimensions_and_actor_order_are_deterministic() {
        let mut s = scene();
        s.actors[0].emotion = "neutral".into();
        for (v, half) in [
            (Viseme::A, [7.0, 7.0]),
            (Viseme::O, [4.0, 5.5]),
            (Viseme::U, [4.0, 5.5]),
            (Viseme::E, [9.0, 3.0]),
            (Viseme::I, [9.0, 3.0]),
            (Viseme::Rest, [6.5, 1.0]),
            (Viseme::Closed, [6.5, 1.0]),
            (Viseme::TeethLip, [6.0, 2.5]),
            (Viseme::TongueTeeth, [6.0, 2.5]),
            (Viseme::TongueUp, [6.0, 2.5]),
            (Viseme::Sibilant, [6.0, 2.5]),
            (Viseme::Sh, [6.0, 2.5]),
            (Viseme::R, [6.0, 2.5]),
        ] {
            let c = build(&s, 640, 360, &[("EMMA", v)], Labels::OmitAcknowledged).unwrap();
            assert_eq!(c.commands[9].0[2][2..], half);
        }
        let mut second = s.actors[0].clone();
        second.id = "ALEX".into();
        second.x += 100.0;
        s.actors.push(second);
        let c = build(&s, 640, 360, &[], Labels::OmitAcknowledged).unwrap();
        assert_eq!(c.command_count(), 20);
        assert_ne!(c.commands[0].0[1], c.commands[10].0[1]);
    }
}
