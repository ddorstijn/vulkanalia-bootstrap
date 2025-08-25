use std::sync::Arc;
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

            let instance = InstanceBuilder::new(Some(window.clone()))
                .app_name("Example Vulkan Application")
                .engine_name("Example Vulkan Engine")
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
    let event_loop = EventLoop::new()?;
    let mut app = App::default();
    event_loop.run_app(&mut app)?;

    Ok(())
}
