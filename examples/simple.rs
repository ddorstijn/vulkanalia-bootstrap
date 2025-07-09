use ash_bootstrap::{
    DeviceBuilder, InstanceBuilder, PhysicalDeviceSelector, PreferredDeviceType, QueueType,
    SwapchainBuilder,
};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::sync::Arc;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

#[derive(Default, Debug)]
struct App {
    window: Option<Window>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let init_window = || -> anyhow::Result<Window> {
            let window = event_loop.create_window(WindowAttributes::default())?;

            let window_handle = window.window_handle()?;
            let display_handle = window.display_handle().unwrap();

            let instance = InstanceBuilder::new(None)
                .app_name("Example Vulkan Application")
                .engine_name("Example Vulkan Engine")
                .request_validation_layers(true)
                .use_default_tracing_messenger()
                .build()?;

            let physical_device = PhysicalDeviceSelector::new(instance.clone())
                .preferred_device_type(PreferredDeviceType::Discrete)
                .select()?;

            let device = Arc::new(DeviceBuilder::new(physical_device, instance.clone()).build()?);

            // You can get the inner handle that is used by vulkan
            // Or you can just pass it where the device handle is expected, because it implements AsRef.
            let _device_handle = device.handle();

            let (_graphics_queue_index, _graphics_queue) = device.get_queue(QueueType::Graphics)?;
            let swapchain_builder = SwapchainBuilder::new(instance.clone(), device.clone());

            let swapchain = swapchain_builder.build()?;

            // And right now we got rid of 400-500 lines of vulkan boilerplate just like that.
            // Now let's cleanup.

            swapchain.destroy();
            device.destroy();
            instance.destroy();

            Ok(window)
        };

        if let Ok(window) = init_window() {
            self.window.replace(window);
        } else {
            panic!("Could not initialize window")
        }
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        _event: WindowEvent,
    ) {
    }
}

fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let event_loop = EventLoop::new()?;
    let mut app = App::default();
    event_loop.run_app(&mut app)?;

    Ok(())
}
