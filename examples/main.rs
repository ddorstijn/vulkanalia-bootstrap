use ash::{khr, vk, Entry};
use std::time::{Duration, Instant};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::application::ApplicationHandler;
use winit::event::{KeyEvent, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{Key, NamedKey};
use winit::window::{Window, WindowAttributes, WindowId};
use ash_bootstrap::{DeviceBuilder, InstanceBuilder, PhysicalDeviceSelector};

pub struct App {
    window: Option<Window>,
}

impl App {
    pub fn new() -> Self {
        Self {
            window: None,
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let monitor = event_loop.primary_monitor().unwrap();
        let window = event_loop
            .create_window(
                WindowAttributes::default(),
            )
            .unwrap();

        let instance = InstanceBuilder::new(Some((window.window_handle().unwrap(), window.display_handle().unwrap())))
            .request_validation_layers(true)
            .app_name("test")
            .engine_name("xolaani")
            .use_default_tracing_messenger()
            .add_debug_messenger_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                    | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
            )
            .minimum_instance_version(vk::make_api_version(0, 1, 3, 0))
            .build()
            .unwrap();

        let physical_device_selector = PhysicalDeviceSelector::new(&instance)
            .allow_any_gpu_device_type(false);

        let mut physical_device = physical_device_selector
            //.select_first_device_unconditionally(true)
            .select()
            .unwrap();

        physical_device.enable_extensions_if_present([
            vk::KHR_DYNAMIC_RENDERING_NAME.to_string_lossy(),
            vk::KHR_DEPTH_STENCIL_RESOLVE_NAME.to_string_lossy(),
            vk::KHR_CREATE_RENDERPASS2_NAME.to_string_lossy(),
            vk::KHR_MULTIVIEW_NAME.to_string_lossy(),
            vk::KHR_MAINTENANCE2_NAME.to_string_lossy(),
        ]);

        println!("{physical_device:#?}");

        let device_builder = DeviceBuilder::new(&physical_device, &instance)
            .build()
            .unwrap();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        event_loop.exit()
    }
}

fn main() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    tracing::trace!("HI!");

    let mut app = App::new();
    let ev = EventLoop::new().unwrap();

    ev.run_app(&mut app).unwrap();
}