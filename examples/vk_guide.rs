use std::sync::Arc;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};
use vulkanalia::vk::HasBuilder;
use vulkanalia::{Version, vk};
use vulkanalia_bootstrap::{
    DeviceBuilder, InstanceBuilder, PhysicalDeviceSelector, PreferredDeviceType, QueueType,
    SwapchainBuilder,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

#[derive(Default, Debug)]
struct App {
    window: Option<Arc<Window>>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let init_window = || -> anyhow::Result<Arc<Window>> {
            let window = Arc::new(event_loop.create_window(WindowAttributes::default())?);

            // Create an instance with some sensible defaults (request validation layers in debug)
            let instance = InstanceBuilder::new(Some(window.clone()))
                .app_name("vk-guide example")
                .engine_name("vulkanalia-bootstrap")
                .request_validation_layers(true)
                .minimum_instance_version(Version::new(1, 3, 0))
                .require_api_version(Version::new(1, 3, 0))
                .build()?;

            let features12 = vk::PhysicalDeviceVulkan12Features::builder()
                .buffer_device_address(true)
                .descriptor_indexing(true);

            let features13 = vk::PhysicalDeviceVulkan13Features::builder()
                .synchronization2(true)
                .dynamic_rendering(true);

            let physical_device = PhysicalDeviceSelector::new(instance.clone())
                .preferred_device_type(PreferredDeviceType::Discrete)
                .add_required_extension_feature(*features12)
                .add_required_extension_feature(*features13)
                .select()?;

            let device = Arc::new(DeviceBuilder::new(physical_device, instance.clone()).build()?);

            // Acquire the graphics queue to validate queue selection
            let (_graphics_queue_index, _graphics_queue) = device.get_queue(QueueType::Graphics)?;

            // Build a swapchain for the window+device
            let swapchain_builder = SwapchainBuilder::new(instance.clone(), device.clone())
                .use_default_format_selection()
                .use_default_present_modes();

            let swapchain = swapchain_builder.build()?;

            // Query images and create image views using the library helper.
            let images = swapchain.get_images()?;
            println!(
                "Swapchain has {} images, extent: {}x{}",
                images.len(),
                swapchain.extent.width,
                swapchain.extent.height
            );

            // Use the library-provided helper to create image views. The implementation was fixed
            // to construct ImageViewCreateInfo correctly, so this should be safe across platforms.
            let image_views = swapchain.get_image_views()?;
            println!("Created {} image views", image_views.len());

            // Destroy image views via the swapchain helper before destroying the swapchain/device
            swapchain.destroy_image_views().ok();

            // Cleanup and destroy swapchain/device/instance
            swapchain.destroy();
            device.destroy();
            instance.destroy();

            Ok(window)
        };

        match init_window() {
            Ok(window) => {
                self.window.replace(window);
            }
            Err(e) => {
                panic!("Could not initialize window: {}", e);
            }
        }
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            _ => (),
        }
    }
}

fn main() -> anyhow::Result<()> {
    // Initialize a simple tracing subscriber so example logs are visible
    tracing_subscriber::registry()
        .with(fmt::layer())
        .with(EnvFilter::from_default_env())
        .init();

    let event_loop = EventLoop::new()?;
    let mut app = App::default();
    event_loop.run_app(&mut app)?;

    Ok(())
}
