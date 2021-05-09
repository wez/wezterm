use ::window::*;
use anyhow::Context;
use promise::spawn::spawn;
#[cfg(target_os = "macos")]
use raw_window_handle::{HasRawWindowHandle, RawWindowHandle};

pub struct GpuContext {
    pub swap_chain: wgpu::SwapChain,
    pub sc_desc: wgpu::SwapChainDescriptor,
    pub adapter: wgpu::Adapter,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface,
}

struct MyWindow {
    allow_close: bool,
    cursor_pos: Point,
    dims: Dimensions,
    gpu: Option<GpuContext>,
    render_pipeline: Option<wgpu::RenderPipeline>,
}

impl Drop for MyWindow {
    fn drop(&mut self) {
        eprintln!("MyWindow dropped");
    }
}

impl MyWindow {
    async fn enable_wgpu(&mut self, win: &Window) -> anyhow::Result<()> {
        let instance = wgpu::Instance::new(if cfg!(target_os = "macos") {
            wgpu::BackendBit::METAL
        } else if cfg!(windows) {
            // Vulkan supports window opacity, but DX12 doesn't
            wgpu::BackendBit::PRIMARY
            // wgpu::BackendBit::DX12
        } else {
            wgpu::BackendBit::PRIMARY
        });

        let surface = unsafe { instance.create_surface(win) };
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: Some(&surface),
            })
            .await
            .ok_or_else(|| anyhow::anyhow!("No suitable GPU adapters found on the system!"))?;

        let adapter_info = adapter.get_info();
        log::info!("wgpu adapter: {:?}", adapter_info);

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    features: wgpu::Features::empty(),
                    limits: wgpu::Limits::default(),
                },
                None,
            )
            .await
            .context("Unable to find a suitable GPU adapter!")?;

        log::info!("wgpu device features: {:?}", device.features());
        log::info!("wgpu device limits: {:?}", device.limits());

        let format = adapter
            .get_swap_chain_preferred_format(&surface)
            .ok_or_else(|| anyhow::anyhow!("adapter is not compatible with surface"))?;

        let sc_desc = wgpu::SwapChainDescriptor {
            usage: wgpu::TextureUsage::RENDER_ATTACHMENT,
            format,
            width: self.dims.pixel_width as u32,
            height: self.dims.pixel_height as u32,
            present_mode: wgpu::PresentMode::Mailbox,
        };
        let swap_chain = device.create_swap_chain(&surface, &sc_desc);

        let gpu = GpuContext {
            swap_chain,
            sc_desc,
            adapter,
            device,
            queue,
            surface,
        };

        let shader = gpu
            .device
            .create_shader_module(&wgpu::ShaderModuleDescriptor {
                label: None,
                source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                    "shader.wgsl"
                ))),
                flags: wgpu::ShaderFlags::all(),
            });

        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: None,
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

        self.render_pipeline
            .replace(
                gpu.device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: None,
                        layout: Some(&pipeline_layout),
                        vertex: wgpu::VertexState {
                            module: &shader,
                            entry_point: "vs_main",
                            buffers: &[],
                        },
                        fragment: Some(wgpu::FragmentState {
                            module: &shader,
                            entry_point: "fs_main",
                            targets: &[format.into()],
                        }),
                        primitive: wgpu::PrimitiveState::default(),
                        depth_stencil: None,
                        multisample: wgpu::MultisampleState::default(),
                    }),
            );

        self.gpu.replace(gpu);
        Ok(())
    }

    fn paint(&mut self) -> anyhow::Result<()> {
        if let Some(gpu) = self.gpu.as_mut() {
            let frame = match gpu.swap_chain.get_current_frame() {
                Ok(frame) => frame,
                Err(err) => {
                    log::info!("get_current_frame: {:#}", err);
                    gpu.swap_chain = gpu.device.create_swap_chain(&gpu.surface, &gpu.sc_desc);
                    gpu.swap_chain
                        .get_current_frame()
                        .expect("Failed to acquire next swap chain texture!")
                }
            };

            let mut encoder = gpu
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            {
                let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[wgpu::RenderPassColorAttachment {
                        view: &frame.output.view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.,
                                g: 0.,
                                b: 0.5,
                                a: 0.5,
                            }),
                            store: true,
                        },
                    }],
                    depth_stencil_attachment: None,
                });
                rpass.set_pipeline(self.render_pipeline.as_ref().unwrap());
                rpass.draw(0..3, 0..1);
            }

            gpu.queue.submit(Some(encoder.finish()));
        }

        Ok(())
    }

    fn resize(&mut self, dims: Dimensions) {
        if self.dims == dims {
            // May just be a move event
            return;
        }
        self.dims = dims;
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.sc_desc.width = dims.pixel_width as u32;
            gpu.sc_desc.height = dims.pixel_height as u32;
            gpu.swap_chain = gpu.device.create_swap_chain(&gpu.surface, &gpu.sc_desc);
        }
    }
}

async fn spawn_window() -> anyhow::Result<()> {
    let (win, events) = Window::new_window("myclass", "the title", 800, 600, None).await?;

    let mut state = MyWindow {
        allow_close: false,
        cursor_pos: Point::new(100, 200),
        dims: Dimensions {
            pixel_width: 800,
            pixel_height: 600,
            dpi: 0,
        },
        gpu: None,
        render_pipeline: None,
    };

    eprintln!("before show");
    win.show().await?;
    state.enable_wgpu(&win).await?;
    eprintln!("window is visible, do loop");

    while let Ok(event) = events.recv().await {
        match event {
            WindowEvent::CloseRequested => {
                eprintln!("can I close?");
                if state.allow_close {
                    win.close();
                } else {
                    state.allow_close = true;
                }
            }
            WindowEvent::Destroyed => {
                eprintln!("destroy was called!");
                Connection::get().unwrap().terminate_message_loop();
            }
            WindowEvent::Resized {
                dimensions,
                is_full_screen: _,
            } => {
                state.resize(dimensions);
                #[cfg(target_os = "macos")]
                if let RawWindowHandle::MacOS(h) = win.raw_window_handle() {
                    use cocoa::base::{id, NO};
                    use objc::*;
                    unsafe {
                        // Allow transparency, as the default for Metal is opaque
                        let layer: id = msg_send![h.ns_view as id, layer];
                        let () = msg_send![layer, setOpaque: NO];
                    }

                    state.paint()?;
                }
            }
            WindowEvent::MouseEvent(event) => {
                state.cursor_pos = event.coords;
                win.invalidate();
                win.set_cursor(Some(MouseCursor::Arrow));

                if event.kind == MouseEventKind::Press(MousePress::Left) {
                    eprintln!("{:?}", event);
                }
            }
            WindowEvent::KeyEvent(key) => {
                eprintln!("{:?}", key);
                win.set_cursor(Some(MouseCursor::Text));
                win.default_key_processing(key);
            }
            WindowEvent::NeedRepaint => {
                state.paint()?;
            }
            WindowEvent::Notification(_) | WindowEvent::FocusChanged(_) => {}
        }
    }

    Ok(())
}

fn main() -> anyhow::Result<()> {
    let _ = pretty_env_logger::formatted_builder()
        .filter_level(log::LevelFilter::Info)
        .try_init();
    let conn = Connection::init()?;
    spawn(async {
        eprintln!("running this async block");
        dbg!(spawn_window().await).ok();
        eprintln!("end of async block");
    })
    .detach();
    conn.run_message_loop()
}
