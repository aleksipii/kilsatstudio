//! Genuine evaluated geometry/artwork GPU experiment; no CPU raster frame is uploaded.
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
/// Targets/readback are released after each render. Source image residency has a
/// single bounded, exact-content cache entry, evicted before replacement.
pub struct GpuSceneRenderer {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    info: wgpu::AdapterInfo,
    errors: Arc<Mutex<Vec<String>>>,
    source_pixels: Option<(Vec<u32>, wgpu::Buffer)>,
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

/// Explicit artwork-capable experiment; validates all source/command budgets before
/// requesting any adapter. The caller supplies decoded local assets, not file paths.
#[allow(clippy::too_many_arguments)]
pub fn render_scene_with_art(
    scene: &EvaluatedScene,
    width: u32,
    height: u32,
    mouths: &[(&str, Viseme)],
    labels: Labels,
    scene_id: &str,
    document: &motion_art::Document,
    images: &std::collections::BTreeMap<String, motion_art::ImageResource>,
    backend: Backend,
    cancel: &AtomicBool,
) -> Result<Frame, Error> {
    let commands = scene_commands::build_with_art(scene, width, height, mouths, labels, scene_id, document, images)?;
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
            source_pixels: None,
        })
    }

    pub fn render(&mut self, scene: &SceneCommands, cancel: &AtomicBool) -> Result<Frame, Error> {
        let deadline = Instant::now() + Duration::from_secs(10);
        gpu::check(cancel, deadline)?;
        let (width, height) = scene.dimensions();
        let (padded, bytes) = gpu::row_layout(width, height)?;
        // SceneCommands has private fields: only the checked builder constructs it.
        // At 2048 square target+readback+returned pixels are 48 MiB. Bounded source
        // pixels, resident host/device copies and transient upload add at most
        // 64 MiB; vertices/commands remain below the 128 MiB owned-resource target.
        // Caller-owned decoded assets and command requests are separate budgets.
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
        let point_bytes: Vec<u8> = scene.points.iter().flatten().flat_map(|v| v.to_le_bytes()).collect();
        let auxiliary = |name: &'static str, bytes: &[u8]| {
            let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(name),
                size: (bytes.len() as u64).max(8),
                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            if !bytes.is_empty() { self.queue.write_buffer(&buffer, 0, bytes); }
            buffer
        };
        let points = auxiliary("bounded artwork vertices", &point_bytes);
        // One content-checked resident set: exact bytes prevent stale asset-ID reuse.
        // Replacement evicts the previous buffer before allocating the next one.
        if self.source_pixels.as_ref().is_none_or(|(content, _)| content != &scene.pixels) {
            self.source_pixels = None;
            let pixel_bytes: Vec<u8> = scene.pixels.iter().flat_map(|v| v.to_le_bytes()).collect();
            let buffer = auxiliary("bounded source image pixels", &pixel_bytes);
            self.source_pixels = Some((scene.pixels.clone(), buffer));
        }
        let pixels = &self.source_pixels.as_ref().ok_or(Error::Budget)?.1;
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
                wgpu::BindGroupEntry { binding: 2, resource: points.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: pixels.as_entire_binding() },
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
struct Shape { inverse: vec4<f32>, origin: vec4<f32>, geometry: vec4<f32>, color: vec4<f32>, parameters: vec4<f32>, };
struct Scene { canvas: vec4<f32>, background: vec4<f32>, settings: vec4<f32>, };
@group(0) @binding(0) var<storage, read> shapes: array<Shape>;
@group(0) @binding(1) var<uniform> scene: Scene;
@group(0) @binding(2) var<storage, read> points: array<vec2<f32>>;
@group(0) @binding(3) var<storage, read> pixels: array<u32>;
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
 if s.parameters.x < .5 { return length(p-g.xy)-g.z; }
 if s.parameters.x < 1.5 { return box_distance(p-g.xy,g.zw-vec2(s.parameters.y))-s.parameters.y; }
 let delta=g.zw-g.xy;
 let len=length(delta);
 if len < .000001 { return length(p-g.xy)-s.parameters.y; }
 if s.parameters.x < 2.5 {
  let t=clamp(dot(p-g.xy,delta)/dot(delta,delta),0.,1.);
  return length(p-g.xy-t*delta)-s.parameters.y;
 }
 let axis=delta/len;
 let v=p-(g.xy+g.zw)*.5;
 return box_distance(vec2(dot(v,axis),dot(v,vec2(-axis.y,axis.x))),vec2(len*.5,s.parameters.y));
}
fn polygon(p: vec2<f32>, offset: u32, count: u32) -> f32 {
 var winding=0i;
 var distance=10000000.;
 for(var i=0u; i<count; i=i+1u) {
  let a=points[offset+i]; let b=points[offset+(i+1u)%count];
  let edge=b-a; let delta=p-a;
  let t=clamp(dot(delta,edge)/max(dot(edge,edge),.0000001),0.,1.);
  distance=min(distance,length(delta-edge*t));
  let cross=(b.x-a.x)*(p.y-a.y)-(p.x-a.x)*(b.y-a.y);
  if a.y<=p.y && b.y>p.y && cross>0. { winding=winding+1i; }
  if a.y>p.y && b.y<=p.y && cross<0. { winding=winding-1i; }
 }
 return select(distance,-distance,winding!=0i);
}
fn image_pixel(offset: u32, size: vec2<u32>, position: vec2<i32>) -> vec4<f32> {
 let xy=vec2<u32>(clamp(position,vec2(0),vec2<i32>(size)-vec2(1)));
 let value=pixels[offset+xy.y*size.x+xy.x];
 let color=vec4<f32>(f32(value&255u),f32((value>>8u)&255u),f32((value>>16u)&255u),f32(value>>24u))/255.;
 return vec4(color.rgb*color.a,color.a);
}
fn image_color(p: vec2<f32>, s: Shape) -> vec4<f32> {
 let uv=(p-s.geometry.xy)/s.geometry.zw;
 if any(uv<vec2(0.)) || any(uv>=vec2(1.)) { return vec4(0.); }
 let size=vec2<u32>(s.parameters.zw);
 let q=uv*vec2<f32>(size)-vec2(.5);
 let low=vec2<i32>(floor(q)); let f=fract(q); let offset=u32(s.parameters.y);
 let a=mix(image_pixel(offset,size,low),image_pixel(offset,size,low+vec2(1,0)),f.x);
 let b=mix(image_pixel(offset,size,low+vec2(0,1)),image_pixel(offset,size,low+vec2(1,1)),f.x);
 let value=mix(a,b,f.y);
 return vec4(value.rgb/max(value.a,.0000001),value.a);
}
fn over(source: vec4<f32>, backdrop: vec4<f32>, mode: f32) -> vec4<f32> {
 // Source is straight RGBA; backdrop is premultiplied, byte-space sRGB.
 let background=backdrop.rgb/max(backdrop.a,.0000001);
 var mixed=source.rgb;
 if mode>.5 && mode<1.5 { mixed=source.rgb*background; }
 if mode>1.5 { mixed=source.rgb+background-source.rgb*background; }
 let rgb=source.a*((1.-backdrop.a)*source.rgb+backdrop.a*mixed)+(1.-source.a)*backdrop.rgb;
 return vec4(rgb,source.a+(1.-source.a)*backdrop.a);
}
@fragment fn fs(@builtin(position) pixel: vec4<f32>) -> @location(0) vec4<f32> {
 let p=pixel.xy;
 var canvas=scene.canvas;
 var phase_opacity=scene.settings.x;
 var active=all(p>=canvas.xy) && all(p<canvas.xy+canvas.zw);
 var first_color=vec3(0.);
 var second_weight=1.;
 var colors: array<vec4<f32>,16>;
 var masks: array<f32,16>;
 var opacities: array<f32,16>;
 var blends: array<f32,16>;
 colors[0]=vec4(scene.background.rgb,1.);
 var depth=0u;
 for (var i=0u; i<u32(scene.settings.y); i=i+1u) {
  let s=shapes[i]; let kind=s.parameters.x;
  if kind>13.5 && kind<14.5 {
   second_weight=s.origin.y;
   first_color=floor(clamp(select(vec3(0.),colors[0].rgb*phase_opacity,active),vec3(0.),vec3(1.))*255.+vec3(.5))/255.*(1.-second_weight);
   canvas=s.geometry; phase_opacity=s.origin.x;
   active=all(p>=canvas.xy) && all(p<canvas.xy+canvas.zw);
   colors[0]=vec4(s.color.rgb,1.); depth=0u; continue;
  }
  if !active { continue; }
  let local=vec2(s.inverse.x*p.x+s.inverse.z*p.y,s.inverse.y*p.x+s.inverse.w*p.y)+s.origin.xy;
  if kind>7.5 && kind<8.5 {
   depth=depth+1u; colors[depth]=vec4(0.); masks[depth]=1.;
   opacities[depth]=s.geometry.x; blends[depth]=s.geometry.y; continue;
  }
  if kind>11.5 && kind<12.5 {
   let value=colors[depth];
   let straight=vec4(value.rgb/max(value.a,.0000001),value.a*masks[depth]*opacities[depth]);
   let mode=blends[depth]; depth=depth-1u;
   colors[depth]=over(straight,colors[depth],mode); continue;
  }
  if kind>8.5 && kind<9.5 {
   let distance=box_distance(local-(s.geometry.xy+s.geometry.zw*.5),s.geometry.zw*.5);
   masks[depth]=masks[depth]*(1.-smoothstep(-s.origin.z,s.origin.z,distance)); continue;
  }
  if kind>9.5 && kind<10.5 {
   let radius=s.geometry.zw*.5; let radial=length((local-s.geometry.xy-radius)/radius);
   let feather=s.parameters.y;
   var coverage=1.-smoothstep(1.-s.origin.z/min(radius.x,radius.y),1.+s.origin.z/min(radius.x,radius.y),radial);
   if feather>0. { coverage=clamp((1.-radial)/feather,0.,1.); }
   masks[depth]=masks[depth]*coverage; continue;
  }
  if kind>10.5 && kind<11.5 {
   let distance=polygon(local,u32(s.geometry.x),u32(s.geometry.y));
   masks[depth]=masks[depth]*(1.-smoothstep(-s.origin.z,s.origin.z,distance)); continue;
  }
  if kind>4.5 && kind<5.5 { colors[depth]=over(image_color(local,s),colors[depth],0.); continue; }
  var distance=0.;
  if kind>3.5 && kind<4.5 {
   let radius=s.geometry.zw*.5;
   distance=(length((local-s.geometry.xy-radius)/radius)-1.)*min(radius.x,radius.y);
  } else if kind>5.5 && kind<6.5 {
   distance=10000000.;
   let offset=u32(s.geometry.x); let count=u32(s.geometry.y);
   for(var j=1u;j<count;j=j+1u) {
    let a=points[offset+j-1u]; let delta=points[offset+j]-a;
    let t=clamp(dot(local-a,delta)/max(dot(delta,delta),.0000001),0.,1.);
    distance=min(distance,length(local-a-delta*t)-s.geometry.z);
   }
  } else if kind>6.5 && kind<7.5 { distance=polygon(local,u32(s.geometry.x),u32(s.geometry.y));
  } else { distance=distance_to_shape(local,s); }
  let coverage=1.-smoothstep(-s.origin.z,s.origin.z,distance);
  colors[depth]=over(vec4(s.color.rgb,coverage*s.color.a),colors[depth],0.);
 }
 let second_color=floor(clamp(select(vec3(0.),colors[0].rgb*phase_opacity,active),vec3(0.),vec3(1.))*255.+vec3(.5))/255.;
 return vec4(floor((first_color+second_color*second_weight)*255.+vec3(.5))/255.,1.);
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
        for p in reference.as_chunks_mut::<4>().0 {
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
