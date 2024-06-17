use std::num::NonZeroU32;
use std::ops::Deref;
use std::sync::Arc;

use glutin::api::egl::device::Device;
use glutin::api::egl::display::Display;
use glutin::config::{ConfigSurfaceTypes, ConfigTemplate, ConfigTemplateBuilder, GlConfig};
use glutin::context::{ContextApi, ContextAttributesBuilder, NotCurrentGlContext, PossiblyCurrentGlContext};
use glutin::display::{GetGlDisplay, GlDisplay};
use skia_safe::{ColorType, gpu, ImageInfo, ISize, Surface};
use skia_safe::gpu::gl::FramebufferInfo;
use skia_safe::gpu::SurfaceOrigin;
use softbuffer::SoftBufferError;
use winit::dpi::PhysicalSize;
use winit::window::Window;
use crate::impl_window_wrapper;

pub struct WindowWrapper {
    skia_context: gpu::DirectContext,
    skia_surface: Option<Surface>,
    soft_buffer_context: softbuffer::Context<Arc<Window>>,
    soft_buffer_surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    size: ISize,
}

impl WindowWrapper {
    pub fn wrap(window: Window) -> Self {
        let devices = Device::query_devices().expect("Failed to query devices").collect::<Vec<_>>();

        for (index, device) in devices.iter().enumerate() {
            println!(
                "Device {}: Name: {} Vendor: {}",
                index,
                device.name().unwrap_or("UNKNOWN"),
                device.vendor().unwrap_or("UNKNOWN")
            );
        }

        let device = devices.first().expect("No available devices");

        // Create a display using the device.
        let display =
            unsafe { Display::with_device(device, None) }.expect("Failed to create display");

        let template = config_template();
        let config = unsafe { display.find_configs(template) }
            .unwrap()
            .reduce(
                |config, acc| {
                    if config.num_samples() > acc.num_samples() {
                        config
                    } else {
                        acc
                    }
                },
            )
            .expect("No available configs");

        println!("Picked a config with {} samples", config.num_samples());

        // Context creation.
        //
        // In particular, since we are doing offscreen rendering we have no raw window
        // handle to provide.
        let context_attributes = ContextAttributesBuilder::new().build(None);

        // Since glutin by default tries to create OpenGL core context, which may not be
        // present we should try gles.
        let fallback_context_attributes =
            ContextAttributesBuilder::new().with_context_api(ContextApi::OpenGl(None)).build(None);

        let not_current = unsafe {
            display.create_context(&config, &context_attributes).unwrap_or_else(|_| {
                display
                    .create_context(&config, &fallback_context_attributes)
                    .expect("failed to create context")
            })
        };

        // Make the context current for rendering
        let context = not_current.make_current_surfaceless().unwrap();
        println!("Context created: {:?}", context.is_current());


        let interface = gpu::gl::Interface::new_load_with_cstr(|name|{
            context.display().get_proc_address(name)
        }).unwrap();



        let window = Arc::new(window);
        let soft_buffer_context = softbuffer::Context::new(window.clone()).unwrap();
        let soft_buffer_surface = softbuffer::Surface::new(&soft_buffer_context, window.clone()).unwrap();
        Self {
            skia_context: gpu::direct_contexts::make_gl(interface, None).unwrap(),
            skia_surface: None,
            soft_buffer_context,
            soft_buffer_surface,
            size: Default::default(),
        }
    }

    fn create_surface(&mut self, size: impl Into<PhysicalSize<u32>>) -> Surface {
        let size = size.into();
        let width = size.width;
        let height = size.height;
        let image_info = ImageInfo::new_n32_premul((width as i32, height as i32), None);
        gpu::surfaces::render_target(
            &mut self.skia_context,
            gpu::Budgeted::Yes,
            &image_info,
            None,
            SurfaceOrigin::TopLeft,
            None,
            false,
            None,
        ).unwrap()
    }
}

fn config_template() -> ConfigTemplate {
    ConfigTemplateBuilder::default()
        .with_alpha_size(8)
        // Offscreen rendering has no support window surface support.
        .with_surface_type(ConfigSurfaceTypes::empty())
        .build()
}

impl_window_wrapper!();