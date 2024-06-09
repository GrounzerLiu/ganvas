use std::num::NonZeroU32;
use std::ops::Deref;
use std::sync::Arc;
use skia_safe::{gpu, ImageInfo, ISize, Surface};
use softbuffer::SoftBufferError;
use winit::dpi::PhysicalSize;
use winit::window::Window;

use offscreen_gl_context::{GLContext, GLVersion, NativeGLContext, sparkle};

pub struct WindowWrapper {
    skia_context: gpu::DirectContext,
    skia_surface: Option<Surface>,
    soft_buffer_context: softbuffer::Context<Arc<Window>>,
    soft_buffer_surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    size: ISize,
}

impl WindowWrapper {
    pub fn wrap(window: Window) -> Self {
        let gl_context = GLContext::<NativeGLContext>::create(
            sparkle::gl::GlType::Gl,
            GLVersion::MajorMinor(3, 3),
            None,
        )
            .unwrap();

        gl_context.make_current().unwrap();

        let interface = gpu::gl::Interface::new_native().unwrap();

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

    pub fn resize(&mut self, size: impl Into<PhysicalSize<u32>>) -> Result<(), SoftBufferError>{
        let size = size.into();
        let width = NonZeroU32::new(size.width).unwrap();
        let height = NonZeroU32::new(size.height).unwrap();
        let result=self.soft_buffer_surface.resize(width, height);
        match result {
            Ok(_) => {
                let surface = self.create_surface(size);
                self.skia_surface = Some(surface);
                self.size = ISize::new(size.width as i32, size.height as i32);
                Ok(())
            }
            Err(e) => {
                return Err(e)
            }
        }
    }

    pub fn surface(&mut self) -> &mut Surface {
        if let Some(surface) = &mut self.skia_surface {
            surface
        } else {
            panic!("Surface not created. Please call resize first.");
        }
    }

    fn create_surface(&mut self, size: impl Into<PhysicalSize<u32>>) -> Surface {
        let size = size.into();
        let width = size.width;
        let height = size.height;
        let image_info = ImageInfo::new_n32_premul((width as i32, height as i32), None);
        let mut surface = gpu::surfaces::render_target(
            &mut self.skia_context,
            gpu::Budgeted::Yes,
            &image_info,
            None,
            gpu::SurfaceOrigin::TopLeft,
            None,
            false,
            None,
        )
            .unwrap();
        surface
    }

    pub fn present(&mut self){
        if let Some(surface) = &mut self.skia_surface {
            let mut soft_buffer = self.soft_buffer_surface.buffer_mut().unwrap();
            let u8_slice = bytemuck::cast_slice_mut::<u32, u8>(&mut soft_buffer);
            let image_info = ImageInfo::new_n32_premul((self.size.width, self.size.height), None);
            surface.read_pixels(
                &image_info,
                u8_slice,
                self.size.width as usize * 4,
                (0, 0),
            );
            soft_buffer.present().unwrap();
        }
    }
}

impl AsRef<Window> for WindowWrapper {
    fn as_ref(&self) -> &Window {
        self.soft_buffer_surface.window()
    }
}

impl Deref for WindowWrapper {
    type Target = Window;

    fn deref(&self) -> &Self::Target {
        self.soft_buffer_surface.window()
    }
}