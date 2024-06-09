use std::{ffi::CString, os::raw, ptr};
use std::num::NonZeroU32;
use std::ops::Deref;
use std::sync::Arc;

use ash::{
    Entry,
    Instance as AshInstance, vk::{self, Handle},
};
use skia_safe::{gpu, ImageInfo, ISize, Surface};
use softbuffer::SoftBufferError;
use winit::dpi::PhysicalSize;
use winit::window::Window;

pub struct WindowWrapper {
    skia_context: gpu::DirectContext,
    skia_surface: Option<Surface>,
    ash_graphics: AshGraphics,
    soft_buffer_context: softbuffer::Context<Arc<Window>>,
    soft_buffer_surface: softbuffer::Surface<Arc<Window>, Arc<Window>>,
    size: ISize
}

impl WindowWrapper {
    pub fn wrap(window: Window) -> Self {
        let ash_graphics = unsafe { AshGraphics::new("skia-org") };
        let skia_context = {
            let get_proc = |of| unsafe {
                match ash_graphics.get_proc(of) {
                    Some(f) => f as _,
                    None => {
                        println!("resolve of {} failed", of.name().to_str().unwrap());
                        ptr::null()
                    }
                }
            };

            let backend_context = unsafe {
                gpu::vk::BackendContext::new(
                    ash_graphics.instance.handle().as_raw() as _,
                    ash_graphics.physical_device.as_raw() as _,
                    ash_graphics.device.handle().as_raw() as _,
                    (
                        ash_graphics.queue_and_index.0.as_raw() as _,
                        ash_graphics.queue_and_index.1,
                    ),
                    &get_proc,
                )
            };

            gpu::direct_contexts::make_vulkan(&backend_context, None).unwrap()
        };

        let window = Arc::new(window);
        let soft_buffer_context = softbuffer::Context::new(window.clone()).unwrap();
        let mut soft_buffer_surface = softbuffer::Surface::new(&soft_buffer_context, window).unwrap();

        Self {
            skia_context,
            skia_surface: None,
            ash_graphics,
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

pub struct AshGraphics {
    pub entry: Entry,
    pub instance: AshInstance,
    pub physical_device: vk::PhysicalDevice,
    pub device: ash::Device,
    pub queue_and_index: (vk::Queue, usize),
}

impl Drop for AshGraphics {
    fn drop(&mut self) {
        unsafe {
            self.device.device_wait_idle().unwrap();
            self.device.destroy_device(None);
            self.instance.destroy_instance(None);
        }
    }
}

// most code copied from here: https://github.com/MaikKlein/ash/blob/master/examples/src/lib.rs
impl AshGraphics {
    pub fn vulkan_version() -> Option<(usize, usize, usize)> {
        let entry = unsafe { Entry::load() }.unwrap();

        let detected_version = unsafe { entry.try_enumerate_instance_version().unwrap_or(None) };

        detected_version.map(|ver| {
            (
                vk::api_version_major(ver).try_into().unwrap(),
                vk::api_version_minor(ver).try_into().unwrap(),
                vk::api_version_patch(ver).try_into().unwrap(),
            )
        })
    }

    pub unsafe fn new(app_name: &str) -> AshGraphics {
        let entry = Entry::load().unwrap();

        // Minimum version supported by Skia.
        let minimum_version = vk::make_api_version(0, 1, 1, 0);

        let instance: AshInstance = {
            let api_version = Self::vulkan_version()
                .map(|(major, minor, patch)| {
                    vk::make_api_version(
                        0,
                        major.try_into().unwrap(),
                        minor.try_into().unwrap(),
                        patch.try_into().unwrap(),
                    )
                })
                .unwrap_or(minimum_version);

            let app_name = CString::new(app_name).unwrap();
            let layer_names: [&CString; 0] = [];
            // let layer_names: [&CString; 1] = [&CString::new("VK_LAYER_LUNARG_standard_validation").unwrap()];
            let extension_names_raw = [
                // These extensions are needed to support MoltenVK on macOS.
                vk::KHR_GET_PHYSICAL_DEVICE_PROPERTIES2_NAME.as_ptr(),
                vk::KHR_PORTABILITY_ENUMERATION_NAME.as_ptr(),
            ];

            let app_info = vk::ApplicationInfo::default()
                .application_name(&app_name)
                .application_version(0)
                .engine_name(&app_name)
                .engine_version(0)
                .api_version(api_version);

            let layers_names_raw: Vec<*const raw::c_char> = layer_names
                .iter()
                .map(|raw_name| raw_name.as_ptr())
                .collect();

            let create_info = vk::InstanceCreateInfo::default()
                .application_info(&app_info)
                .enabled_layer_names(&layers_names_raw)
                .enabled_extension_names(&extension_names_raw)
                // Flag is needed to support MoltenVK on macOS.
                .flags(vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR);

            entry
                .create_instance(&create_info, None)
                .expect("Failed to create a Vulkan instance")
        };

        let (physical_device, queue_family_index) = {
            let physical_devices = instance
                .enumerate_physical_devices()
                .expect("Failed to enumerate Vulkan physical devices");

            physical_devices
                .iter()
                .map(|physical_device| {
                    instance
                        .get_physical_device_queue_family_properties(*physical_device)
                        .iter()
                        .enumerate()
                        .find_map(|(index, info)| {
                            let supports_graphic =
                                info.queue_flags.contains(vk::QueueFlags::GRAPHICS);
                            supports_graphic.then_some((*physical_device, index))
                        })
                })
                .find_map(|v| v)
                .expect("Failed to find a suitable Vulkan device")
        };

        let device: ash::Device = {
            let features = vk::PhysicalDeviceFeatures::default();

            let priorities = [1.0];

            let queue_info = [vk::DeviceQueueCreateInfo::default()
                .queue_family_index(queue_family_index as _)
                .queue_priorities(&priorities)];

            let device_extension_names_raw = [];

            let device_create_info = vk::DeviceCreateInfo::default()
                .queue_create_infos(&queue_info)
                .enabled_extension_names(&device_extension_names_raw)
                .enabled_features(&features);

            instance
                .create_device(physical_device, &device_create_info, None)
                .unwrap()
        };

        let queue_index: usize = 0;
        let queue: vk::Queue = device.get_device_queue(queue_family_index as _, queue_index as _);

        AshGraphics {
            queue_and_index: (queue, queue_index),
            device,
            physical_device,
            instance,
            entry,
        }
    }

    pub unsafe fn get_proc(&self, of: gpu::vk::GetProcOf) -> Option<unsafe extern "system" fn()> {
        match of {
            gpu::vk::GetProcOf::Instance(instance, name) => {
                let ash_instance = vk::Instance::from_raw(instance as _);
                self.entry.get_instance_proc_addr(ash_instance, name)
            }
            gpu::vk::GetProcOf::Device(device, name) => {
                let ash_device = vk::Device::from_raw(device as _);
                self.instance.get_device_proc_addr(ash_device, name)
            }
        }
    }
}