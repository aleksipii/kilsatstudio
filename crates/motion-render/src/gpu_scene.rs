//! Genuine evaluated-geometry GPU experiment; no CPU raster frame is uploaded.
//! This renderer is intentionally absent from production preview/export selection.
//! Font labels require explicit omission acknowledgement. Analytical antialiasing
//! and smile joins differ from SVG; full-image CPU/GPU parity is not established.
use crate::gpu::{self, Backend, Frame};
use crate::scene_commands::{self, Error, Labels, SceneCommands};
use motion_lipsync::Viseme;
use motion_scene::EvaluatedScene;
use std::sync::{atomic::AtomicBool, Arc, Mutex};
use std::time::{Duration, Instant};

/// Reuses the adapter, device, queue and pipeline; only one render can be in flight.
/// Targets/readback are released after each render, so resizing cannot grow a cache.
pub struct GpuSceneRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    info: wgpu::AdapterInfo,
    errors: Arc<Mutex<Vec<String>>>,
}

/// Validate evaluated geometry, all dimensions and work budgets before adapter
/// initialization. Call off the UI thread. No fallback to CPU rendering exists.
pub fn render_scene(
    scene: &EvaluatedScene,
    width: u32,
    height: u32,
    mouths: &[(&str, Viseme)],
    labels: Labels,
    backend: Backend,
    cancel: &AtomicBool,
) -> Result<Frame, Error> {
    let commands = scene_commands::build(scene, width, height, mouths, labels)?;
    gpu::check(cancel, Instant::now() + Duration::from_secs(10))?;
    GpuSceneRenderer::new(backend, cancel)?.render(&commands, cancel)
}

impl GpuSceneRenderer {
    /// Explicit backend only; rejects software/unknown adapters. Native driver
    /// calls are not forcibly interruptible; use the existing child watchdog for
    /// physical device acceptance. Initialization/readback polling is cooperative.
    pub fn new(backend: Backend, cancel: &AtomicBool) -> Result<Self, Error> {
        let deadline = Instant::now() + Duration::from_secs(10);
        gpu::check(cancel, deadline)?;
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: backend.mask(),
            ..Default::default()
        });
        let adapter = gpu::wait(
            instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: false,
            }),
            cancel,
            deadline,
        )?
        .ok_or(gpu::Error::NoAdapter)?;
        let info = adapter.get_info();
        if !backend.mask().contains(info.backend.into())
            || matches!(
                info.device_type,
                wgpu::DeviceType::Cpu | wgpu::DeviceType::Other
            )
        {
            return Err(gpu::Error::NotHardware.into());
        }
        let (device, queue) = gpu::wait(
            adapter.request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Kilsat evaluated geometry experiment"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::MemoryUsage,
                },
                None,
            ),
            cancel,
            deadline,
        )?
        .map_err(|e| gpu::Error::Device(e.to_string()))?;
        let errors = Arc::new(Mutex::new(Vec::new()));
        let captured = errors.clone();
        device.on_uncaptured_error(Box::new(move |error| {
            if let Ok(mut errors) = captured.lock() {
                if errors.len() < 8 {
                    errors.push(error.to_string());
                }
            }
        }));
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("evaluated geometry analytic shapes"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("experimental evaluated scene"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        if let Some(error) = gpu::wait(device.pop_error_scope(), cancel, deadline)? {
            return Err(gpu::Error::Device(error.to_string()).into());
        }
        Ok(Self {
            device,
            queue,
            pipeline,
            info,
            errors,
        })
    }

    pub fn render(&mut self, scene: &SceneCommands, cancel: &AtomicBool) -> Result<Frame, Error> {
        let deadline = Instant::now() + Duration::from_secs(10);
        gpu::check(cancel, deadline)?;
        let (width, height) = scene.dimensions();
        let (padded, bytes) = gpu::row_layout(width, height)?;
        // SceneCommands has private fields: only the checked builder constructs it.
        // At 2048 square target+readback+returned pixels are 48 MiB; commands
        // max out at 320 KiB each on host/device, well below the 128 MiB budget.
        let device = &self.device;
        // Per-frame errors use the bounded uncaptured-error collector. There is
        // no per-frame scope to leak if cancellation interrupts allocation or
        // readback; initialization retains its separate validation scope.
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("bounded evaluated scene target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let mut commands = scene.bytes();
        // WGSL storage arrays require at least one element even with count zero.
        if commands.is_empty() {
            commands.resize(80, 0);
        }
        let shapes = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bounded scene commands"),
            size: commands.len() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&shapes, 0, &commands);
        let uniform_values = [
            scene.canvas,
            scene.background,
            [
                scene.opacity,
                scene.command_count() as f32,
                width as f32,
                height as f32,
            ],
        ];
        let uniform_bytes: Vec<u8> = uniform_values
            .iter()
            .flatten()
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene composition"),
            size: uniform_bytes.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        self.queue.write_buffer(&uniforms, 0, &uniform_bytes);
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: shapes.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: uniforms.as_entire_binding(),
                },
            ],
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("scene bounded readback"),
            size: bytes,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let view = texture.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("real evaluated geometry composition"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..3, 0..1);
        }
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        gpu::check(cancel, deadline)?;
        self.queue.submit(Some(encoder.finish()));
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        let result = (|| -> Result<Vec<u8>, gpu::Error> {
            loop {
                gpu::check(cancel, deadline)?;
                device.poll(wgpu::Maintain::Poll);
                match receiver.try_recv() {
                    Ok(result) => {
                        result.map_err(|e| gpu::Error::Device(e.to_string()))?;
                        break;
                    }
                    Err(std::sync::mpsc::TryRecvError::Empty) => {
                        std::thread::sleep(Duration::from_millis(1))
                    }
                    Err(error) => return Err(gpu::Error::Device(error.to_string())),
                }
            }
            let mapped = readback.slice(..).get_mapped_range();
            let mut rgba = Vec::with_capacity((width * height * 4) as usize);
            for row in mapped.chunks_exact(padded as usize) {
                rgba.extend_from_slice(&row[..(width * 4) as usize]);
            }
            drop(mapped);
            readback.unmap();
            Ok(rgba)
        })();
        let rgba = result?;
        let errors = self
            .errors
            .lock()
            .map_err(|e| gpu::Error::Device(e.to_string()))?
            .clone();
        if !errors.is_empty() {
            return Err(gpu::Error::Device(errors.join("; ")).into());
        }
        Ok(Frame {
            width,
            height,
            rgba,
            adapter_name: self.info.name.clone(),
            backend: format!("{:?}", self.info.backend),
            device_type: format!("{:?}", self.info.device_type),
            validation_errors: errors,
        })
    }
}

/// Decoded byte-space diagnostics, deliberately without a pass/fail tolerance.
/// Edge pixels are reference pixels with a >8 channel difference to a direct
/// neighbour. Retain color-space/alpha conventions alongside these measurements;
/// this calculation does not establish perceptual or linear-light equivalence.
#[derive(Clone, Debug, PartialEq)]
pub struct ImageDifference {
    pub maximum_channel_error: [u8; 4],
    pub rms_channel_error: [f64; 4],
    pub changed_pixel_fraction: f64,
    pub edge_pixels: u64,
    pub edge_rms_error: f64,
    pub interior_rms_error: f64,
}

pub fn compare_decoded_rgba(
    width: u32,
    height: u32,
    reference: &[u8],
    candidate: &[u8],
) -> Result<ImageDifference, Error> {
    gpu::row_layout(width, height)?;
    let pixels = u64::from(width) * u64::from(height);
    if reference.len() as u64 != pixels * 4 || candidate.len() != reference.len() {
        return Err(gpu::Error::Pixels("decoded RGBA dimensions/length mismatch".into()).into());
    }
    let mut maxima = [0u8; 4];
    let mut sums = [0u64; 4];
    let mut changed = 0u64;
    let mut edge_pixels = 0u64;
    let mut edge_sum = 0u64;
    let mut interior_sum = 0u64;
    for y in 0..height {
        for x in 0..width {
            let i = ((y * width + x) * 4) as usize;
            let edge = [
                (x.saturating_sub(1), y),
                ((x + 1).min(width - 1), y),
                (x, y.saturating_sub(1)),
                (x, (y + 1).min(height - 1)),
            ]
            .into_iter()
            .any(|(nx, ny)| {
                let n = ((ny * width + nx) * 4) as usize;
                (0..4).any(|c| reference[i + c].abs_diff(reference[n + c]) > 8)
            });
            let mut squared = 0u64;
            let mut differs = false;
            for c in 0..4 {
                let difference = reference[i + c].abs_diff(candidate[i + c]);
                differs |= difference != 0;
                maxima[c] = maxima[c].max(difference);
                let square = u64::from(difference).pow(2);
                sums[c] += square;
                squared += square;
            }
            changed += u64::from(differs);
            if edge {
                edge_pixels += 1;
                edge_sum += squared;
            } else {
                interior_sum += squared;
            }
        }
    }
    let rms = |sum: u64, count: u64| {
        if count == 0 {
            0.0
        } else {
            (sum as f64 / (count as f64 * 4.0)).sqrt()
        }
    };
    Ok(ImageDifference {
        maximum_channel_error: maxima,
        rms_channel_error: sums.map(|sum| (sum as f64 / pixels as f64).sqrt()),
        changed_pixel_fraction: changed as f64 / pixels as f64,
        edge_pixels,
        edge_rms_error: rms(edge_sum, edge_pixels),
        interior_rms_error: rms(interior_sum, pixels - edge_pixels),
    })
}

const SHADER: &str = r#"
struct Shape { inverse: vec4<f32>, origin: vec4<f32>, geometry: vec4<f32>, color: vec4<f32>, meta: vec4<f32>, };
struct Scene { canvas: vec4<f32>, background: vec4<f32>, settings: vec4<f32>, };
@group(0) @binding(0) var<storage, read> shapes: array<Shape>;
@group(0) @binding(1) var<uniform> scene: Scene;
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
 var positions = array<vec2<f32>,3>(vec2(-1.,-1.),vec2(3.,-1.),vec2(-1.,3.));
 return vec4(positions[i],0.,1.);
}
fn box_distance(p: vec2<f32>, half_size: vec2<f32>) -> f32 {
 let q=abs(p)-half_size;
 return length(max(q,vec2(0.)))+min(max(q.x,q.y),0.);
}
fn distance_to_shape(p: vec2<f32>, s: Shape) -> f32 {
 let g=s.geometry;
 if s.meta.x < .5 { return length(p-g.xy)-g.z; }
 if s.meta.x < 1.5 { return box_distance(p-g.xy,g.zw-vec2(s.meta.y))-s.meta.y; }
 let delta=g.zw-g.xy;
 let len=length(delta);
 if len < .000001 { return length(p-g.xy)-s.meta.y; }
 if s.meta.x < 2.5 {
  let t=clamp(dot(p-g.xy,delta)/dot(delta,delta),0.,1.);
  return length(p-g.xy-t*delta)-s.meta.y;
 }
 let axis=delta/len;
 let v=p-(g.xy+g.zw)*.5;
 return box_distance(vec2(dot(v,axis),dot(v,vec2(-axis.y,axis.x))),vec2(len*.5,s.meta.y));
}
@fragment fn fs(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
 let p=pixel.xy;
 let canvas=scene.canvas;
 if any(p<canvas.xy) || any(p>=canvas.xy+canvas.zw) { return vec4(0.,0.,0.,1.); }
 var color=scene.background.rgb;
 for (var i=0u; i<u32(scene.settings.y); i=i+1u) {
  let s=shapes[i];
  let local=vec2(s.inverse.x*p.x+s.inverse.z*p.y,s.inverse.y*p.x+s.inverse.w*p.y)+s.origin.xy;
  let coverage=1.-smoothstep(-s.origin.z,s.origin.z,distance_to_shape(local,s));
  color=mix(color,s.color.rgb,coverage*s.color.a);
 }
 return vec4(color*scene.settings.x,1.);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn evaluated_geometry_shader_validates_without_adapter() {
        let module = wgpu::naga::front::wgsl::parse_str(SHADER).unwrap();
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap();
    }
    #[test]
    fn decoded_full_image_comparison_reports_interior_and_edges() {
        let mut reference = vec![128u8; 64 * 64 * 4];
        for p in reference.chunks_exact_mut(4) {
            p[3] = 255;
        }
        let center = (32 * 64 + 32) * 4;
        reference[center] = 255;
        let mut candidate = reference.clone();
        candidate[center] = 200;
        candidate[0] = 100;
        let result = compare_decoded_rgba(64, 64, &reference, &candidate).unwrap();
        assert_eq!(result.maximum_channel_error, [55, 0, 0, 0]);
        assert_eq!(result.changed_pixel_fraction, 2.0 / 4096.0);
        assert_eq!(result.edge_pixels, 5);
        assert!(result.edge_rms_error > 0.0);
        assert!(result.interior_rms_error > 0.0);
        let exact = compare_decoded_rgba(64, 64, &reference, &reference).unwrap();
        assert_eq!(exact.changed_pixel_fraction, 0.0);
        assert_eq!(exact.rms_channel_error, [0.0; 4]);
        assert!(compare_decoded_rgba(64, 64, &reference, &candidate[..100]).is_err());
    }
    #[test]
    fn cancellation_and_unsupported_labels_precede_device_initialization() {
        let scene = crate::tests::scene();
        let cancel = AtomicBool::new(true);
        assert!(matches!(
            render_scene(
                &scene,
                640,
                360,
                &[],
                Labels::Require,
                Backend::Metal,
                &cancel
            ),
            Err(Error::UnsupportedFeature(_))
        ));
        assert!(matches!(
            render_scene(
                &scene,
                640,
                360,
                &[],
                Labels::OmitAcknowledged,
                Backend::Metal,
                &cancel
            ),
            Err(Error::Gpu(gpu::Error::Cancelled))
        ));
    }
}
