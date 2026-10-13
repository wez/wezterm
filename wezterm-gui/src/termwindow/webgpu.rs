use crate::quad::Vertex;
use anyhow::anyhow;
use config::{ConfigHandle, GpuInfo, WebGpuPowerPreference};
use std::cell::RefCell;
use std::path::Path;
use std::sync::Arc;
use wgpu::util::DeviceExt;
use window::bitmaps::Texture2d;
use window::raw_window_handle::{
    DisplayHandle, HandleError, HasDisplayHandle, HasWindowHandle, RawDisplayHandle,
    RawWindowHandle, WindowHandle,
};
use window::{BitmapImage, Dimensions, Rect, Window};

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ShaderUniform {
    pub foreground_text_hsb: [f32; 3],
    pub milliseconds: u32,
    pub projection: [[f32; 4]; 4],
    // sampler2D atlas_nearest_sampler;
    // sampler2D atlas_linear_sampler;
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PostProcessUniform {
    pub resolution: [f32; 2],
    pub time: f32,
    pub time_delta: f32,
    pub frame: u32,
    pub _padding: [u32; 3],
}

/// State for the post-processing shader pipeline.
/// Stored on TermWindow directly (not inside WebGpuState) to avoid
/// Rc/RefCell complexity when accessing the wgpu device.
pub struct PostProcessState {
    pub intermediate_texture: wgpu::Texture,
    pub intermediate_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub _uniform_bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group_intermediate: wgpu::BindGroup,
    pub bind_group_pingpong: Option<wgpu::BindGroup>,
    pub uniform_buffer: wgpu::Buffer,
    pub uniform_bind_group: wgpu::BindGroup,
    pub pipelines: Vec<wgpu::RenderPipeline>,
    pub ping_pong_texture: Option<wgpu::Texture>,
    pub ping_pong_view: Option<wgpu::TextureView>,
    /// The surface format used when these resources were created,
    /// so we can detect if it changes on resize.
    pub format: wgpu::TextureFormat,
}

/// The preamble from postprocess.wgsl minus the default fs_postprocess function.
/// User shaders only need to provide `fn fs_postprocess(in: VertexOutput) -> @location(0) vec4<f32>`.
const POSTPROCESS_PREAMBLE: &str = "\
struct PostProcessUniform {
    resolution: vec2<f32>,
    time: f32,
    time_delta: f32,
    frame: u32,
    _padding_0: u32,
    _padding_1: u32,
    _padding_2: u32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@group(0) @binding(0) var screen_texture: texture_2d<f32>;
@group(0) @binding(1) var screen_sampler: sampler;

@group(1) @binding(0) var<uniform> pp: PostProcessUniform;

@vertex
fn vs_postprocess(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    var out: VertexOutput;
    let x = f32(i32(vertex_index & 1u) * 4 - 1);
    let y = f32(i32(vertex_index >> 1u) * 4 - 1);
    out.position = vec4<f32>(x, y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return out;
}

";

/// Strip BOM, decode UTF-8, validate non-empty, and prepend the standard
/// preamble. Returns the full WGSL source ready for compilation, or None
/// on any input problem (logged).
pub(crate) fn prepare_shader_source(raw_bytes: &[u8], path: &Path) -> Option<String> {
    // Handle BOM and decode as UTF-8
    let source_str = if raw_bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        // UTF-8 BOM — skip it
        match std::str::from_utf8(&raw_bytes[3..]) {
            Ok(s) => s.to_string(),
            Err(e) => {
                log::error!(
                    "postprocess: shader file {} is not valid UTF-8: {:#}",
                    path.display(),
                    e
                );
                return None;
            }
        }
    } else {
        match std::str::from_utf8(raw_bytes) {
            Ok(s) => s.to_string(),
            Err(e) => {
                log::error!(
                    "postprocess: shader file {} is not valid UTF-8: {:#}",
                    path.display(),
                    e
                );
                return None;
            }
        }
    };

    let trimmed = source_str.trim();
    if trimmed.is_empty() {
        log::error!(
            "postprocess: shader file {} is empty",
            path.display()
        );
        return None;
    }

    // Prepend the standard preamble (structs, bindings, vertex shader)
    // so user only needs to define fs_postprocess
    Some(format!("{}{}", POSTPROCESS_PREAMBLE, source_str))
}

/// Compile a single post-process shader from a file path.
/// Returns None on any failure (missing file, bad WGSL, pipeline error)
/// without crashing — errors are logged.
fn compile_postprocess_shader(
    device: &wgpu::Device,
    path: &Path,
    format: wgpu::TextureFormat,
    texture_bind_group_layout: &wgpu::BindGroupLayout,
    uniform_bind_group_layout: &wgpu::BindGroupLayout,
) -> Option<wgpu::RenderPipeline> {
    let raw_source = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            log::error!(
                "postprocess: failed to read shader file {}: {:#}",
                path.display(),
                e
            );
            return None;
        }
    };

    let full_source = prepare_shader_source(&raw_source, path)?;

    // Use error scopes to catch validation errors without crashing
    device.push_error_scope(wgpu::ErrorFilter::Validation);

    let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some(&format!("PostProcess Shader: {}", path.display())),
        source: wgpu::ShaderSource::Wgsl(full_source.into()),
    });

    // Poll for validation errors from shader compilation
    let shader_error = smol::block_on(device.pop_error_scope());
    if let Some(err) = shader_error {
        log::error!(
            "postprocess: shader compilation failed for {}: {:#}",
            path.display(),
            err
        );
        return None;
    }

    // Build the pipeline layout: group 0 = texture+sampler, group 1 = uniform
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some(&format!("PostProcess Pipeline Layout: {}", path.display())),
        bind_group_layouts: &[texture_bind_group_layout, uniform_bind_group_layout],
        push_constant_ranges: &[],
    });

    device.push_error_scope(wgpu::ErrorFilter::Validation);

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(&format!("PostProcess Pipeline: {}", path.display())),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader_module,
            entry_point: Some("vs_postprocess"),
            buffers: &[], // fullscreen triangle, no vertex buffers
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader_module,
            entry_point: Some("fs_postprocess"),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend: None, // post-process replaces pixels, no blending
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: wgpu::PipelineCompilationOptions::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: None,
            polygon_mode: wgpu::PolygonMode::Fill,
            unclipped_depth: false,
            conservative: false,
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: 1,
            mask: !0,
            alpha_to_coverage_enabled: false,
        },
        multiview: None,
        cache: None,
    });

    let pipeline_error = smol::block_on(device.pop_error_scope());
    if let Some(err) = pipeline_error {
        log::error!(
            "postprocess: render pipeline creation failed for {}: {:#}",
            path.display(),
            err
        );
        return None;
    }

    log::info!(
        "postprocess: successfully compiled shader {}",
        path.display()
    );
    Some(pipeline)
}

impl PostProcessState {
    /// Create the full post-process state: intermediate textures, bind groups,
    /// compiled shader pipelines. Returns None if no shaders compiled successfully
    /// or if dimensions are zero.
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        shader_paths: &[std::path::PathBuf],
    ) -> Option<Self> {
        if width == 0 || height == 0 {
            log::warn!("postprocess: skipping creation with zero dimensions");
            return None;
        }

        if shader_paths.is_empty() {
            return None;
        }

        // Build bind group layouts first so we can compile pipelines
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("PostProcess texture bind group layout"),
            });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("PostProcess uniform bind group layout"),
            });

        // Compile all shader pipelines, skipping any that fail
        let pipelines: Vec<wgpu::RenderPipeline> = shader_paths
            .iter()
            .filter_map(|path| {
                compile_postprocess_shader(
                    device,
                    path,
                    format,
                    &texture_bind_group_layout,
                    &uniform_bind_group_layout,
                )
            })
            .collect();

        if pipelines.is_empty() {
            log::error!(
                "postprocess: no shaders compiled successfully out of {} configured",
                shader_paths.len()
            );
            return None;
        }

        log::info!(
            "postprocess: {} of {} shaders compiled successfully",
            pipelines.len(),
            shader_paths.len()
        );

        // Create intermediate texture — same format as surface
        let intermediate_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("PostProcess Intermediate Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let intermediate_view =
            intermediate_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // Sampler for reading intermediate/ping-pong textures
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        // Uniform buffer
        let uniform_buffer =
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("PostProcess Uniform Buffer"),
                contents: bytemuck::cast_slice(&[PostProcessUniform::default()]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("PostProcess Uniform Bind Group"),
        });

        // Pre-create bind group for reading the intermediate texture
        let bind_group_intermediate =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &texture_bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&intermediate_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
                label: Some("PostProcess Intermediate Bind Group"),
            });

        // Ping-pong texture only needed when >1 shader in the chain
        let (ping_pong_texture, ping_pong_view, bind_group_pingpong) = if pipelines.len() > 1 {
            let pp_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("PostProcess Ping-Pong Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let pp_view =
                pp_texture.create_view(&wgpu::TextureViewDescriptor::default());

            let pp_bind_group =
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &texture_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&pp_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                    label: Some("PostProcess Ping-Pong Bind Group"),
                });

            (Some(pp_texture), Some(pp_view), Some(pp_bind_group))
        } else {
            (None, None, None)
        };

        Some(PostProcessState {
            intermediate_texture,
            intermediate_view,
            sampler,
            bind_group_layout: texture_bind_group_layout,
            _uniform_bind_group_layout: uniform_bind_group_layout,
            bind_group_intermediate,
            bind_group_pingpong,
            uniform_buffer,
            uniform_bind_group,
            pipelines,
            ping_pong_texture,
            ping_pong_view,
            format,
        })
    }

    /// Recreate textures and bind groups at new dimensions.
    /// Preserves compiled pipelines (they are dimension-independent).
    /// Returns false if dimensions are zero (caller should drop state).
    pub fn resize(&mut self, device: &wgpu::Device, width: u32, height: u32) -> bool {
        if width == 0 || height == 0 {
            log::warn!("postprocess: resize called with zero dimensions, skipping");
            return false;
        }

        let format = self.format;

        // Recreate intermediate texture
        self.intermediate_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("PostProcess Intermediate Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        self.intermediate_view = self
            .intermediate_texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        // Recreate bind group for intermediate
        self.bind_group_intermediate =
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&self.intermediate_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
                label: Some("PostProcess Intermediate Bind Group"),
            });

        // Recreate ping-pong if present
        if self.ping_pong_texture.is_some() {
            let pp_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("PostProcess Ping-Pong Texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            });
            let pp_view =
                pp_texture.create_view(&wgpu::TextureViewDescriptor::default());

            self.bind_group_pingpong =
                Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &self.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&pp_view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.sampler),
                        },
                    ],
                    label: Some("PostProcess Ping-Pong Bind Group"),
                }));

            self.ping_pong_texture = Some(pp_texture);
            self.ping_pong_view = Some(pp_view);
        }

        true
    }
}

pub struct WebGpuState {
    pub adapter_info: wgpu::AdapterInfo,
    pub downlevel_caps: wgpu::DownlevelCapabilities,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: Arc<wgpu::Queue>,
    pub config: RefCell<wgpu::SurfaceConfiguration>,
    pub dimensions: RefCell<Dimensions>,
    pub render_pipeline: wgpu::RenderPipeline,
    shader_uniform_bind_group_layout: wgpu::BindGroupLayout,
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
    pub texture_nearest_sampler: wgpu::Sampler,
    pub texture_linear_sampler: wgpu::Sampler,
    pub handle: RawHandlePair,
}

pub struct RawHandlePair {
    window: RawWindowHandle,
    display: RawDisplayHandle,
}

impl RawHandlePair {
    fn new(window: &Window) -> Self {
        Self {
            window: window.window_handle().expect("window handle").as_raw(),
            display: window.display_handle().expect("display handle").as_raw(),
        }
    }
}

impl HasWindowHandle for RawHandlePair {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        unsafe { Ok(WindowHandle::borrow_raw(self.window)) }
    }
}

impl HasDisplayHandle for RawHandlePair {
    fn display_handle(&self) -> Result<DisplayHandle<'_>, HandleError> {
        unsafe { Ok(DisplayHandle::borrow_raw(self.display)) }
    }
}

pub struct WebGpuTexture {
    texture: wgpu::Texture,
    width: u32,
    height: u32,
    queue: Arc<wgpu::Queue>,
}

impl std::ops::Deref for WebGpuTexture {
    type Target = wgpu::Texture;
    fn deref(&self) -> &Self::Target {
        &self.texture
    }
}

impl Texture2d for WebGpuTexture {
    fn write(&self, rect: Rect, im: &dyn BitmapImage) {
        let (im_width, im_height) = im.image_dimensions();

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: rect.min_x() as u32,
                    y: rect.min_y() as u32,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            im.pixel_data_slice(),
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(im_width as u32 * 4),
                rows_per_image: Some(im_height as u32),
            },
            wgpu::Extent3d {
                width: im_width as u32,
                height: im_height as u32,
                depth_or_array_layers: 1,
            },
        );
    }

    fn read(&self, _rect: Rect, _im: &mut dyn BitmapImage) {
        unimplemented!();
    }

    fn width(&self) -> usize {
        self.width as usize
    }

    fn height(&self) -> usize {
        self.height as usize
    }
}

impl WebGpuTexture {
    pub fn new(width: u32, height: u32, state: &WebGpuState) -> anyhow::Result<Self> {
        let limit = state.device.limits().max_texture_dimension_2d;

        if width > limit || height > limit {
            // Ideally, wgpu would have a fallible create_texture method,
            // but it doesn't: instead it will panic if the requested
            // dimension is too large.
            // So we check the limit ourselves here.
            // <https://github.com/wezterm/wezterm/issues/3713>
            anyhow::bail!(
                "texture dimensions {width}x{height} exceeed the \
                 max dimension {limit} supported by your GPU"
            );
        }

        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let view_formats = if state
            .downlevel_caps
            .flags
            .contains(wgpu::DownlevelFlags::SURFACE_VIEW_FORMATS)
        {
            vec![format, format.remove_srgb_suffix()]
        } else {
            vec![]
        };
        let texture = state.device.create_texture(&wgpu::TextureDescriptor {
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            label: Some("Texture Atlas"),
            view_formats: &view_formats,
        });
        Ok(Self {
            texture,
            width,
            height,
            queue: Arc::clone(&state.queue),
        })
    }
}

pub fn adapter_info_to_gpu_info(info: wgpu::AdapterInfo) -> GpuInfo {
    GpuInfo {
        name: info.name,
        vendor: Some(info.vendor),
        device: Some(info.device),
        device_type: format!("{:?}", info.device_type),
        driver: if info.driver.is_empty() {
            None
        } else {
            Some(info.driver)
        },
        driver_info: if info.driver_info.is_empty() {
            None
        } else {
            Some(info.driver_info)
        },
        backend: format!("{:?}", info.backend),
    }
}

fn compute_compatibility_list(
    instance: &wgpu::Instance,
    backends: wgpu::Backends,
    surface: &wgpu::Surface,
) -> Vec<String> {
    instance
        .enumerate_adapters(backends)
        .into_iter()
        .map(|a| {
            let info = adapter_info_to_gpu_info(a.get_info());
            let compatible = a.is_surface_supported(&surface);
            format!(
                "{}, compatible={}",
                info.to_string(),
                if compatible { "yes" } else { "NO" }
            )
        })
        .collect()
}

impl WebGpuState {
    pub async fn new(
        window: &Window,
        dimensions: Dimensions,
        config: &ConfigHandle,
    ) -> anyhow::Result<Self> {
        let handle = RawHandlePair::new(window);
        Self::new_impl(handle, dimensions, config).await
    }

    pub async fn new_impl(
        handle: RawHandlePair,
        dimensions: Dimensions,
        config: &ConfigHandle,
    ) -> anyhow::Result<Self> {
        let backends = wgpu::Backends::all();
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });
        let surface = unsafe {
            instance.create_surface_unsafe(wgpu::SurfaceTargetUnsafe::from_window(&handle)?)?
        };

        let mut adapter: Option<wgpu::Adapter> = None;

        if let Some(preference) = &config.webgpu_preferred_adapter {
            for a in instance.enumerate_adapters(backends) {
                if !a.is_surface_supported(&surface) {
                    let info = adapter_info_to_gpu_info(a.get_info());
                    log::warn!("{} is not compatible with surface", info.to_string());
                    continue;
                }

                let info = a.get_info();

                if preference.name != info.name {
                    continue;
                }

                if preference.device_type != format!("{:?}", info.device_type) {
                    continue;
                }

                if preference.backend != format!("{:?}", info.backend) {
                    continue;
                }

                if let Some(driver) = &preference.driver {
                    if *driver != info.driver {
                        continue;
                    }
                }
                if let Some(vendor) = &preference.vendor {
                    if *vendor != info.vendor {
                        continue;
                    }
                }
                if let Some(device) = &preference.device {
                    if *device != info.device {
                        continue;
                    }
                }

                adapter.replace(a);
                break;
            }

            if adapter.is_none() {
                let adapters = compute_compatibility_list(&instance, backends, &surface);
                log::warn!(
                    "Your webgpu preferred adapter '{}' was either not \
                     found or is not compatible with your display. Available:\n{}",
                    preference.to_string(),
                    adapters.join("\n")
                );
            }
        }

        if adapter.is_none() {
            adapter = Some(
                instance
                    .request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: match config.webgpu_power_preference {
                            WebGpuPowerPreference::HighPerformance => {
                                wgpu::PowerPreference::HighPerformance
                            }
                            WebGpuPowerPreference::LowPower => wgpu::PowerPreference::LowPower,
                        },
                        compatible_surface: Some(&surface),
                        force_fallback_adapter: config.webgpu_force_fallback_adapter,
                    })
                    .await?,
            );
        }

        let adapter = adapter.ok_or_else(|| {
            let adapters = compute_compatibility_list(&instance, backends, &surface);
            anyhow!(
                "no compatible adapter found. Available:\n{}",
                adapters.join("\n")
            )
        })?;

        let adapter_info = adapter.get_info();
        log::trace!("Using adapter: {adapter_info:?}");
        let caps = surface.get_capabilities(&adapter);
        log::trace!("caps: {caps:?}");
        let downlevel_caps = adapter.get_downlevel_capabilities();
        log::trace!("downlevel_caps: {downlevel_caps:?}");

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                // WebGL doesn't support all of wgpu's features, so if
                // we're building for the web we'll have to disable some.
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::downlevel_defaults()
                }
                .using_resolution(adapter.limits()),
                label: None,
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await?;

        let queue = Arc::new(queue);

        // Explicitly request an SRGB format, if available
        let pref_format_srgb = caps.formats[0].add_srgb_suffix();
        let format = if caps.formats.contains(&pref_format_srgb) {
            pref_format_srgb
        } else {
            caps.formats[0]
        };

        // Need to check that this is supported, as trying to set
        // view_formats without it will cause surface.configure
        // to panic
        // <https://github.com/wezterm/wezterm/issues/3565>
        let view_formats = if downlevel_caps
            .flags
            .contains(wgpu::DownlevelFlags::SURFACE_VIEW_FORMATS)
        {
            vec![format.add_srgb_suffix(), format.remove_srgb_suffix()]
        } else {
            vec![]
        };

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: dimensions.pixel_width as u32,
            height: dimensions.pixel_height as u32,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: if caps
                .alpha_modes
                .contains(&wgpu::CompositeAlphaMode::PostMultiplied)
            {
                wgpu::CompositeAlphaMode::PostMultiplied
            } else if caps
                .alpha_modes
                .contains(&wgpu::CompositeAlphaMode::PreMultiplied)
            {
                wgpu::CompositeAlphaMode::PreMultiplied
            } else {
                wgpu::CompositeAlphaMode::Auto
            },
            view_formats,
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::include_wgsl!("../shader.wgsl"));

        let shader_uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("ShaderUniform bind group layout"),
            });

        let texture_nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let texture_linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture bind group layout"),
            });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &shader_uniform_bind_group_layout,
                    &texture_bind_group_layout,
                    &texture_bind_group_layout,
                ],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),

            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        Ok(Self {
            adapter_info,
            downlevel_caps,
            surface,
            device,
            queue,
            config: RefCell::new(config),
            dimensions: RefCell::new(dimensions),
            render_pipeline,
            handle,
            shader_uniform_bind_group_layout,
            texture_bind_group_layout,
            texture_nearest_sampler,
            texture_linear_sampler,
        })
    }

    pub fn create_uniform(&self, uniform: ShaderUniform) -> wgpu::BindGroup {
        let buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("ShaderUniform Buffer"),
                contents: bytemuck::cast_slice(&[uniform]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &self.shader_uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
            label: Some("ShaderUniform Bind Group"),
        })
    }

    #[allow(unused_mut)]
    pub fn resize(&self, mut dims: Dimensions) {
        // During a live resize on Windows, the Dimensions that we're processing may be
        // lagging behind the true client size. We have to take the very latest value
        // from the window or else the underlying driver will raise an error about
        // the mismatch, so we need to sneakily read through the handle
        match self.handle.window {
            #[cfg(windows)]
            RawWindowHandle::Win32(h) => {
                let mut rect = unsafe { std::mem::zeroed() };
                unsafe { winapi::um::winuser::GetClientRect(h.hwnd.get() as _, &mut rect) };
                dims.pixel_width = (rect.right - rect.left) as usize;
                dims.pixel_height = (rect.bottom - rect.top) as usize;
            }
            _ => {}
        }

        if dims == *self.dimensions.borrow() {
            return;
        }
        *self.dimensions.borrow_mut() = dims;
        let mut config = self.config.borrow_mut();
        config.width = dims.pixel_width as u32;
        config.height = dims.pixel_height as u32;
        if config.width > 0 && config.height > 0 {
            // Avoid reconfiguring with a 0 sized surface, as webgpu will
            // panic in that case
            // <https://github.com/wezterm/wezterm/issues/2881>
            self.surface.configure(&self.device, &config);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_postprocess_uniform_layout() {
        assert_eq!(
            std::mem::size_of::<PostProcessUniform>(),
            32,
            "PostProcessUniform must be 32 bytes for GPU alignment"
        );
    }

    #[test]
    fn test_preamble_is_valid_wgsl() {
        // Parse the preamble + a minimal valid fragment shader
        let source = format!(
            "{}\n{}",
            POSTPROCESS_PREAMBLE,
            r#"
@fragment
fn fs_postprocess(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(screen_texture, screen_sampler, in.uv);
}
"#
        );
        let result = naga::front::wgsl::parse_str(&source);
        assert!(result.is_ok(), "Preamble + minimal shader must parse: {:?}", result.err());
    }

    #[test]
    fn test_prepare_shader_source_valid() {
        let body = b"@fragment\nfn fs_postprocess(in: VertexOutput) -> @location(0) vec4<f32> {\n    return vec4<f32>(1.0);\n}\n";
        let result = prepare_shader_source(body, &PathBuf::from("test.wgsl"));
        assert!(result.is_some());
        let source = result.unwrap();
        assert!(source.starts_with("struct PostProcessUniform"));
        assert!(source.contains("fn fs_postprocess"));
    }

    #[test]
    fn test_prepare_shader_source_bom() {
        let mut bytes = vec![0xEF, 0xBB, 0xBF]; // UTF-8 BOM
        bytes.extend_from_slice(b"@fragment\nfn fs_postprocess(in: VertexOutput) -> @location(0) vec4<f32> {\n    return vec4<f32>(1.0);\n}\n");
        let result = prepare_shader_source(&bytes, &PathBuf::from("bom.wgsl"));
        assert!(result.is_some());
        let source = result.unwrap();
        // BOM should be stripped, preamble prepended
        assert!(source.starts_with("struct PostProcessUniform"));
        assert!(!source.contains('\u{FEFF}'));
    }

    #[test]
    fn test_prepare_shader_source_empty() {
        assert!(prepare_shader_source(b"", &PathBuf::from("empty.wgsl")).is_none());
        assert!(prepare_shader_source(b"   \n  \t  ", &PathBuf::from("whitespace.wgsl")).is_none());
    }

    #[test]
    fn test_prepare_shader_source_invalid_utf8() {
        let bytes: &[u8] = &[0xFF, 0xFE, 0x80, 0x81];
        assert!(prepare_shader_source(bytes, &PathBuf::from("bad.wgsl")).is_none());
    }

    // --- GPU tests (Tier 2) ---

    fn create_test_bind_group_layouts(
        device: &wgpu::Device,
    ) -> (wgpu::BindGroupLayout, wgpu::BindGroupLayout) {
        let texture_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
            label: Some("Test texture BGL"),
        });

        let uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
            label: Some("Test uniform BGL"),
        });

        (texture_bgl, uniform_bgl)
    }

    fn create_test_device() -> Option<(wgpu::Device, wgpu::Queue)> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        // Try software/fallback adapter first
        let adapter = smol::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::None,
            compatible_surface: None,
            force_fallback_adapter: true,
        }))
        .or_else(|_| {
            smol::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                compatible_surface: None,
                force_fallback_adapter: false,
            }))
        })
        .ok()?;

        let (device, queue) = smol::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults()
                    .using_resolution(adapter.limits()),
                label: Some("Test Device"),
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            },
        ))
        .ok()?;

        Some((device, queue))
    }

    #[test]
    fn test_compile_valid_shader() {
        let Some((device, _queue)) = create_test_device() else {
            eprintln!("Skipping test_compile_valid_shader: no GPU adapter available");
            return;
        };

        let dir = tempfile::tempdir().unwrap();
        let shader_path = dir.path().join("valid.wgsl");
        std::fs::write(
            &shader_path,
            r#"
@fragment
fn fs_postprocess(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(screen_texture, screen_sampler, in.uv);
}
"#,
        )
        .unwrap();

        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let (texture_bgl, uniform_bgl) = create_test_bind_group_layouts(&device);

        let result = compile_postprocess_shader(&device, &shader_path, format, &texture_bgl, &uniform_bgl);
        assert!(result.is_some(), "Valid shader should compile successfully");
    }

    #[test]
    fn test_compile_invalid_shader() {
        let Some((device, _queue)) = create_test_device() else {
            eprintln!("Skipping test_compile_invalid_shader: no GPU adapter available");
            return;
        };

        let dir = tempfile::tempdir().unwrap();
        let shader_path = dir.path().join("invalid.wgsl");
        std::fs::write(&shader_path, "this is not valid WGSL at all!!!").unwrap();

        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let (texture_bgl, uniform_bgl) = create_test_bind_group_layouts(&device);

        let result = compile_postprocess_shader(&device, &shader_path, format, &texture_bgl, &uniform_bgl);
        assert!(result.is_none(), "Invalid shader should return None");
    }

    #[test]
    fn test_compile_missing_entry_point() {
        let Some((device, _queue)) = create_test_device() else {
            eprintln!("Skipping test_compile_missing_entry_point: no GPU adapter available");
            return;
        };

        let dir = tempfile::tempdir().unwrap();
        let shader_path = dir.path().join("no_entry.wgsl");
        // Valid WGSL but wrong function name
        std::fs::write(
            &shader_path,
            r#"
@fragment
fn wrong_name(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(screen_texture, screen_sampler, in.uv);
}
"#,
        )
        .unwrap();

        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let (texture_bgl, uniform_bgl) = create_test_bind_group_layouts(&device);

        let result = compile_postprocess_shader(&device, &shader_path, format, &texture_bgl, &uniform_bgl);
        assert!(result.is_none(), "Missing entry point should return None");
    }

    #[test]
    fn test_compile_missing_file() {
        let Some((device, _queue)) = create_test_device() else {
            eprintln!("Skipping test_compile_missing_file: no GPU adapter available");
            return;
        };

        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let (texture_bgl, uniform_bgl) = create_test_bind_group_layouts(&device);

        let result = compile_postprocess_shader(
            &device,
            &PathBuf::from("/nonexistent/shader.wgsl"),
            format,
            &texture_bgl,
            &uniform_bgl,
        );
        assert!(result.is_none(), "Missing file should return None");
    }

    #[test]
    fn test_postprocess_state_single_shader() {
        let Some((device, _queue)) = create_test_device() else {
            eprintln!("Skipping test_postprocess_state_single_shader: no GPU adapter available");
            return;
        };

        let dir = tempfile::tempdir().unwrap();
        let shader_path = dir.path().join("single.wgsl");
        std::fs::write(
            &shader_path,
            r#"
@fragment
fn fs_postprocess(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(screen_texture, screen_sampler, in.uv);
}
"#,
        )
        .unwrap();

        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let state = PostProcessState::new(&device, format, 800, 600, &[shader_path]);
        assert!(state.is_some(), "Single valid shader should create state");
        let state = state.unwrap();
        assert_eq!(state.pipelines.len(), 1);
        assert!(state.ping_pong_texture.is_none(), "Single shader needs no ping-pong texture");
    }

    #[test]
    fn test_postprocess_state_multi_shader() {
        let Some((device, _queue)) = create_test_device() else {
            eprintln!("Skipping test_postprocess_state_multi_shader: no GPU adapter available");
            return;
        };

        let dir = tempfile::tempdir().unwrap();
        let shader_body = r#"
@fragment
fn fs_postprocess(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(screen_texture, screen_sampler, in.uv);
}
"#;
        let path1 = dir.path().join("s1.wgsl");
        let path2 = dir.path().join("s2.wgsl");
        std::fs::write(&path1, shader_body).unwrap();
        std::fs::write(&path2, shader_body).unwrap();

        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let state = PostProcessState::new(&device, format, 800, 600, &[path1, path2]);
        assert!(state.is_some(), "Two valid shaders should create state");
        let state = state.unwrap();
        assert_eq!(state.pipelines.len(), 2);
        assert!(state.ping_pong_texture.is_some(), "Multi shader needs ping-pong texture");
    }

    #[test]
    fn test_postprocess_state_zero_dimensions() {
        let Some((device, _queue)) = create_test_device() else {
            eprintln!("Skipping test_postprocess_state_zero_dimensions: no GPU adapter available");
            return;
        };

        let dir = tempfile::tempdir().unwrap();
        let shader_path = dir.path().join("z.wgsl");
        std::fs::write(
            &shader_path,
            "@fragment\nfn fs_postprocess(in: VertexOutput) -> @location(0) vec4<f32> { return vec4<f32>(1.0); }\n",
        )
        .unwrap();

        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        assert!(PostProcessState::new(&device, format, 0, 600, &[shader_path.clone()]).is_none());
        assert!(PostProcessState::new(&device, format, 800, 0, &[shader_path]).is_none());
    }

    #[test]
    fn test_postprocess_state_no_valid_shaders() {
        let Some((device, _queue)) = create_test_device() else {
            eprintln!("Skipping test_postprocess_state_no_valid_shaders: no GPU adapter available");
            return;
        };

        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let result = PostProcessState::new(
            &device,
            format,
            800,
            600,
            &[PathBuf::from("/no/such/a.wgsl"), PathBuf::from("/no/such/b.wgsl")],
        );
        assert!(result.is_none(), "No valid shaders should return None");
    }

    #[test]
    fn test_postprocess_state_mixed_valid_invalid() {
        let Some((device, _queue)) = create_test_device() else {
            eprintln!("Skipping test_postprocess_state_mixed_valid_invalid: no GPU adapter available");
            return;
        };

        let dir = tempfile::tempdir().unwrap();
        let valid_path = dir.path().join("good.wgsl");
        std::fs::write(
            &valid_path,
            r#"
@fragment
fn fs_postprocess(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(screen_texture, screen_sampler, in.uv);
}
"#,
        )
        .unwrap();

        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let state = PostProcessState::new(
            &device,
            format,
            800,
            600,
            &[valid_path, PathBuf::from("/no/such/bad.wgsl")],
        );
        assert!(state.is_some(), "Mixed valid/invalid should still create state");
        assert_eq!(state.unwrap().pipelines.len(), 1);
    }

    #[test]
    fn test_postprocess_state_resize() {
        let Some((device, _queue)) = create_test_device() else {
            eprintln!("Skipping test_postprocess_state_resize: no GPU adapter available");
            return;
        };

        let dir = tempfile::tempdir().unwrap();
        let shader_path = dir.path().join("resize.wgsl");
        std::fs::write(
            &shader_path,
            r#"
@fragment
fn fs_postprocess(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(screen_texture, screen_sampler, in.uv);
}
"#,
        )
        .unwrap();

        let format = wgpu::TextureFormat::Bgra8UnormSrgb;
        let mut state = PostProcessState::new(&device, format, 800, 600, &[shader_path]).unwrap();

        assert!(state.resize(&device, 1024, 768), "Resize to valid dims should return true");
        assert!(!state.resize(&device, 0, 768), "Resize to zero width should return false");
        assert!(!state.resize(&device, 1024, 0), "Resize to zero height should return false");
    }

    #[test]
    fn test_shader_actually_renders() {
        let Some((device, queue)) = create_test_device() else {
            eprintln!("Skipping test_shader_actually_renders: no GPU adapter available");
            return;
        };

        let tex_width: u32 = 4;
        let tex_height: u32 = 4;
        let format = wgpu::TextureFormat::Rgba8Unorm;

        // --- Source texture: solid red ---
        let source_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test source texture"),
            size: wgpu::Extent3d {
                width: tex_width,
                height: tex_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let red_pixel: [u8; 4] = [255, 0, 0, 255];
        let pixel_data: Vec<u8> = red_pixel
            .iter()
            .copied()
            .cycle()
            .take((tex_width * tex_height * 4) as usize)
            .collect();

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &source_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixel_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(tex_width * 4),
                rows_per_image: Some(tex_height),
            },
            wgpu::Extent3d {
                width: tex_width,
                height: tex_height,
                depth_or_array_layers: 1,
            },
        );

        let source_view = source_texture.create_view(&wgpu::TextureViewDescriptor::default());

        // --- Render target texture ---
        let render_target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Test render target"),
            size: wgpu::Extent3d {
                width: tex_width,
                height: tex_height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let render_target_view =
            render_target.create_view(&wgpu::TextureViewDescriptor::default());

        // --- Bind group layouts ---
        let (texture_bgl, uniform_bgl) = create_test_bind_group_layouts(&device);

        // --- Sampler ---
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Test sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // --- Texture bind group (group 0) ---
        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Test texture bind group"),
            layout: &texture_bgl,
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

        // --- Uniform buffer + bind group (group 1) ---
        let uniform = PostProcessUniform::default();
        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Test uniform buffer"),
            contents: bytemuck::cast_slice(&[uniform]),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Test uniform bind group"),
            layout: &uniform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // --- Compile the color-inversion shader ---
        let dir = tempfile::tempdir().unwrap();
        let shader_path = dir.path().join("invert.wgsl");
        std::fs::write(
            &shader_path,
            r#"
@fragment
fn fs_postprocess(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(screen_texture, screen_sampler, in.uv);
    return vec4<f32>(1.0 - color.r, 1.0 - color.g, 1.0 - color.b, color.a);
}
"#,
        )
        .unwrap();

        let pipeline =
            compile_postprocess_shader(&device, &shader_path, format, &texture_bgl, &uniform_bgl)
                .expect("Inversion shader should compile");

        // --- Render pass ---
        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Test render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &render_target_view,
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
            rpass.set_pipeline(&pipeline);
            rpass.set_bind_group(0, &texture_bind_group, &[]);
            rpass.set_bind_group(1, &uniform_bind_group, &[]);
            rpass.draw(0..3, 0..1);
        }
        queue.submit(std::iter::once(encoder.finish()));

        // --- Readback: copy render target to staging buffer ---
        let bytes_per_row_unaligned = tex_width * 4;
        // wgpu requires rows aligned to COPY_BYTES_PER_ROW_ALIGNMENT
        let align = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let bytes_per_row = (bytes_per_row_unaligned + align - 1) / align * align;
        let buffer_size = (bytes_per_row * tex_height) as u64;

        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Test staging buffer"),
            size: buffer_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &render_target,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &staging_buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(tex_height),
                },
            },
            wgpu::Extent3d {
                width: tex_width,
                height: tex_height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(std::iter::once(encoder.finish()));

        // Map the buffer and read pixels
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap();
        });
        device.poll(wgpu::PollType::Wait).expect("GPU poll failed");
        receiver
            .recv()
            .expect("Failed to receive map result")
            .expect("Buffer mapping failed");

        let data = buffer_slice.get_mapped_range();

        // Check every pixel in the output (accounting for row padding)
        let tolerance = 2u8;
        let expected: [u8; 4] = [0, 255, 255, 255]; // cyan = inverted red
        for row in 0..tex_height {
            for col in 0..tex_width {
                let offset = (row * bytes_per_row + col * 4) as usize;
                let pixel = &data[offset..offset + 4];
                for (ch, (&got, &exp)) in pixel.iter().zip(expected.iter()).enumerate() {
                    let diff = (got as i16 - exp as i16).unsigned_abs();
                    assert!(
                        diff <= tolerance as u16,
                        "Pixel ({},{}) channel {}: expected {}, got {} (diff {} > tolerance {})",
                        col, row, ch, exp, got, diff, tolerance
                    );
                }
            }
        }

        drop(data);
        staging_buffer.unmap();
    }
}
