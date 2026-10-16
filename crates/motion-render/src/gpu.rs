//! Experimental original offscreen fixture, not a complete scene renderer.
use std::{
    future::Future,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug)]
pub enum Backend {
    Metal,
    Vulkan,
    Gl,
}
impl Backend {
    fn mask(self) -> wgpu::Backends {
        match self {
            Self::Metal => wgpu::Backends::METAL,
            Self::Vulkan => wgpu::Backends::VULKAN,
            Self::Gl => wgpu::Backends::GL,
        }
    }
}
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("fixture dimensions must be 64..=2048 and readback allocation <= 32 MiB")]
    Dimensions,
    #[error("GPU operation cancelled")]
    Cancelled,
    #[error("GPU operation exceeded its polling deadline")]
    Timeout,
    #[error("no adapter for explicitly selected backend")]
    NoAdapter,
    #[error("hardware proof rejects CPU/software or mismatched adapters")]
    NotHardware,
    #[error("GPU failure: {0}")]
    Device(String),
    #[error("fixture pixel comparison failed: {0}")]
    Pixels(String),
}
#[derive(Debug)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub adapter_name: String,
    pub backend: String,
    pub device_type: String,
    pub validation_errors: Vec<String>,
}

pub fn row_layout(width: u32, height: u32) -> Result<(u32, u64), Error> {
    if !(64..=2048).contains(&width) || !(64..=2048).contains(&height) {
        return Err(Error::Dimensions);
    }
    let padded = (width * 4).div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
        * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
    let bytes = u64::from(padded) * u64::from(height);
    if bytes > 32 * 1024 * 1024 {
        return Err(Error::Dimensions);
    }
    Ok((padded, bytes))
}
fn check(cancel: &AtomicBool, deadline: Instant) -> Result<(), Error> {
    if cancel.load(Ordering::Relaxed) {
        Err(Error::Cancelled)
    } else if Instant::now() >= deadline {
        Err(Error::Timeout)
    } else {
        Ok(())
    }
}
struct Signal(std::thread::Thread);
impl Wake for Signal {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}
fn wait<T>(
    future: impl Future<Output = T>,
    cancel: &AtomicBool,
    deadline: Instant,
) -> Result<T, Error> {
    let waker = Waker::from(Arc::new(Signal(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        check(cancel, deadline)?;
        if let Poll::Ready(result) = future.as_mut().poll(&mut context) {
            return Ok(result);
        }
        std::thread::park_timeout(Duration::from_millis(1));
    }
}

/// Call off the UI thread. GPU initialization/driver calls cannot be forcibly
/// preempted; deadline/cancellation bound our future/readback polling loops.
/// `hardware_proof` rejects CPU and nonselected backends, never falling back.
pub fn render_fixture(
    backend: Backend,
    width: u32,
    height: u32,
    hardware_proof: bool,
    cancel: &AtomicBool,
) -> Result<Frame, Error> {
    let (padded, bytes) = row_layout(width, height)?;
    let deadline = Instant::now() + Duration::from_secs(10);
    check(cancel, deadline)?;
    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: backend.mask(),
        ..Default::default()
    });
    let adapter = wait(
        instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: None,
            force_fallback_adapter: false,
        }),
        cancel,
        deadline,
    )?
    .ok_or(Error::NoAdapter)?;
    let info = adapter.get_info();
    if !backend.mask().contains(info.backend.into())
        || (hardware_proof
            && matches!(
                info.device_type,
                wgpu::DeviceType::Cpu | wgpu::DeviceType::Other
            ))
    {
        return Err(Error::NotHardware);
    }
    let (device, queue) = wait(
        adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("Kilsat fixture"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::MemoryUsage,
            },
            None,
        ),
        cancel,
        deadline,
    )?
    .map_err(|e| Error::Device(e.to_string()))?;
    let uncaptured = Arc::new(std::sync::Mutex::new(Vec::new()));
    let captured = uncaptured.clone();
    device.on_uncaptured_error(Box::new(move |error| {
        if let Ok(mut errors) = captured.lock() {
            if errors.len() < 8 {
                errors.push(error.to_string());
            }
        }
    }));
    device.push_error_scope(wgpu::ErrorFilter::Validation);
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("offscreen"),
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
    let texel = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("original orange texel"),
        size: wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        texel.as_image_copy(),
        &[255, 128, 0, 255],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(4),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("fixture geometry"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("fixture"),
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
                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        multiview: None,
        cache: None,
    });
    let source_view = texel.create_view(&Default::default());
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor::default());
    let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&source_view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler),
            },
        ],
    });
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("bounded readback"),
        size: bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let view = texture.create_view(&Default::default());
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("GPU planes"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.1,
                        g: 0.2,
                        b: 0.3,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..6, 0..1);
        pass.draw(6..9, 0..1);
        let x = width * 75 / 100;
        let y = height * 70 / 100;
        pass.set_scissor_rect(x, y, width * 85 / 100 - x, height * 90 / 100 - y);
        pass.draw(9..15, 0..1);
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
    check(cancel, deadline)?;
    queue.submit(Some(encoder.finish()));
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
    loop {
        check(cancel, deadline)?;
        device.poll(wgpu::Maintain::Poll);
        match receiver.try_recv() {
            Ok(result) => {
                result.map_err(|e| Error::Device(e.to_string()))?;
                break;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                std::thread::sleep(Duration::from_millis(1))
            }
            Err(error) => return Err(Error::Device(error.to_string())),
        }
    }
    let mapped = readback.slice(..).get_mapped_range();
    let mut rgba = Vec::with_capacity((width * height * 4) as usize);
    for row in mapped.chunks_exact(padded as usize) {
        rgba.extend_from_slice(&row[..(width * 4) as usize]);
    }
    drop(mapped);
    readback.unmap();
    if let Some(error) = wait(device.pop_error_scope(), cancel, deadline)? {
        return Err(Error::Device(error.to_string()));
    }
    let errors = uncaptured
        .lock()
        .map_err(|e| Error::Device(e.to_string()))?
        .clone();
    if !errors.is_empty() {
        return Err(Error::Device(errors.join("; ")));
    }
    let frame = Frame {
        width,
        height,
        rgba,
        adapter_name: info.name,
        backend: format!("{:?}", info.backend),
        device_type: format!("{:?}", info.device_type),
        validation_errors: errors,
    };
    compare_fixture(&frame)?;
    Ok(frame)
}

/// Interior CPU-reference probes; tolerance 2 per unorm channel. No claim about
/// antialiased edges or full-scene feature parity is made.
pub fn compare_fixture(frame: &Frame) -> Result<(), Error> {
    row_layout(frame.width, frame.height)?;
    if frame.rgba.len() != (frame.width * frame.height * 4) as usize {
        return Err(Error::Pixels("length".into()));
    }
    for (x, y, expected) in [
        (5, 5, [26u8, 51, 77, 255]),
        (20, 20, [255, 128, 0, 255]),
        (55, 40, [0, 255, 0, 255]),
        (80, 80, [13, 26, 166, 255]),
        (70, 80, [26, 51, 77, 255]),
    ] {
        let i = (((frame.height * y / 100) * frame.width + frame.width * x / 100) * 4) as usize;
        if frame.rgba[i..i + 4]
            .iter()
            .zip(expected)
            .any(|(&a, b)| a.abs_diff(b) > 2)
        {
            return Err(Error::Pixels(format!(
                "probe {x}%,{y}%: {:?} expected {expected:?}",
                &frame.rgba[i..i + 4]
            )));
        }
    }
    Ok(())
}
const SHADER: &str = r#"
@group(0) @binding(0) var image: texture_2d<f32>;
@group(0) @binding(1) var image_sampler: sampler;
struct Out { @builtin(position) position: vec4<f32>, @location(0) @interpolate(flat) kind: u32, };
@vertex fn vs(@builtin(vertex_index) i: u32) -> Out {
 var corners = array<vec2<f32>,6>(vec2(0.,0.),vec2(1.,0.),vec2(0.,1.),vec2(0.,1.),vec2(1.,0.),vec2(1.,1.));
 var p: vec2<f32>; var kind: u32;
 if i < 6u { p = vec2(.1,.1) + corners[i]*.3; kind=0u; }
 else if i < 9u {
  var triangle = array<vec2<f32>,3>(vec2(-.12,.12),vec2(.12,.12),vec2(0.,-.12));
  let v=triangle[i-6u]; let a=.4;
  p=vec2(.55,.4)+vec2(cos(a)*v.x-sin(a)*v.y,sin(a)*v.x+cos(a)*v.y); kind=1u;
 } else { p=vec2(.65,.65)+corners[i-9u]*.3; kind=2u; }
 var out: Out; out.position=vec4(p.x*2.-1.,1.-p.y*2.,0.,1.); out.kind=kind; return out;
}
@fragment fn fs(input: Out) -> @location(0) vec4<f32> {
 if input.kind == 0u { return textureSample(image,image_sampler,vec2(.5,.5)); }
 if input.kind == 1u { return vec4(0.,1.,0.,1.); }
 return vec4(0.,0.,1.,.5);
}
"#;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shader_validates_without_adapter() {
        let module = wgpu::naga::front::wgsl::parse_str(SHADER).unwrap();
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .unwrap();
    }
    #[test]
    fn row_padding_and_limits() {
        assert_eq!(row_layout(65, 64).unwrap(), (512, 32768));
        assert!(row_layout(63, 64).is_err());
        assert!(row_layout(2049, 64).is_err());
    }
    #[test]
    fn cancellation_precedes_adapter() {
        let cancel = AtomicBool::new(true);
        assert!(matches!(
            render_fixture(Backend::Metal, 64, 64, true, &cancel),
            Err(Error::Cancelled)
        ));
    }
    #[test]
    fn incomplete_reference_rejected() {
        let f = Frame {
            width: 64,
            height: 64,
            rgba: vec![],
            adapter_name: String::new(),
            backend: String::new(),
            device_type: String::new(),
            validation_errors: vec![],
        };
        assert!(compare_fixture(&f).is_err());
    }
    #[test]
    #[ignore = "requires explicit KILSAT_GPU_BACKEND and actual GPU; never part of default acceptance"]
    fn real_fixture() {
        let backend = match std::env::var("KILSAT_GPU_BACKEND").as_deref() {
            Ok("metal") => Backend::Metal,
            Ok("vulkan") => Backend::Vulkan,
            Ok("gl") => Backend::Gl,
            _ => panic!("set KILSAT_GPU_BACKEND explicitly"),
        };
        for _ in 0..2 {
            render_fixture(backend, 256, 256, true, &AtomicBool::new(false)).unwrap();
        }
    }
}
