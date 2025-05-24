use ash::{khr, vk, Entry};
use std::time::Duration;
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};
use ash_bootstrap::{InstanceBuilder, PhysicalDeviceSelector, SystemInfo};

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
        let monitor = event_loop.primary_monitor().unwrap();
        let window = event_loop
            .create_window(
                WindowAttributes::default(),
            )
            .unwrap();

        let instance = InstanceBuilder::new(Some((window.window_handle().unwrap(), window.display_handle().unwrap())))
            .enable_validation_layers(true)
            .app_name("test")
            .engine_name("xolaani")
            .use_default_tracing_messenger()
            .add_debug_messenger_severity(
                vk::DebugUtilsMessageSeverityFlagsEXT::VERBOSE
                    | vk::DebugUtilsMessageSeverityFlagsEXT::INFO,
            )
            .minimum_instance_version(vk::make_api_version(0, 1, 2, 0))
            .build()
            .unwrap();

        let device = PhysicalDeviceSelector::new(&instance)
            //.select_first_device_unconditionally(true)
            .select()
            .unwrap();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
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