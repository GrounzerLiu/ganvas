use std::num::NonZeroU32;
use std::ops::Deref;
use std::sync::Arc;
use skia_safe::{gpu, ImageInfo, ISize, Surface};
use softbuffer::SoftBufferError;
use winit::dpi::PhysicalSize;
use winit::window::Window;
use crate::impl_window_wrapper;

pub struct WindowWrapper {
    skia_surface: Option<Surface>,
    soft_buffer_context: softbuffer::Context<Arc<Window>>,
    soft_buffer_surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    size: ISize,
}

impl WindowWrapper {
    pub fn wrap(window: Window) -> Self {
        let window = Arc::new(window);
        let soft_buffer_context = softbuffer::Context::new(window.clone()).unwrap();
        let soft_buffer_surface = softbuffer::Surface::new(&soft_buffer_context, window.clone()).unwrap();
        Self {
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
        let surface = skia_safe::surfaces::raster_n32_premul(ISize::new(width as i32, height as i32)).unwrap();
        surface
    }
}

impl_window_wrapper!();