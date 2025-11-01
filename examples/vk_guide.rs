use std::sync::Arc;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};
use vulkanalia::vk::{DeviceV1_0, HasBuilder};
use vulkanalia::{Version, vk};
use vulkanalia_bootstrap::{
    Device, DeviceBuilder, Instance, InstanceBuilder, PhysicalDeviceSelector, PreferredDeviceType,
    QueueType, Swapchain, SwapchainBuilder,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

const FRAME_OVERLAP: usize = 2;

#[derive(Debug)]
struct FrameData {
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    swapchain_semaphore: vk::Semaphore,
    render_semaphore: vk::Semaphore,
    render_fence: vk::Fence,
}

#[derive(Debug)]
struct Vulkan {
    window: Arc<Window>,
    instance: Arc<Instance>,
    device: Arc<Device>,
    swapchain: Swapchain,
    images: Vec<vk::Image>,
    image_views: Vec<vk::ImageView>,

    frames: Vec<FrameData>,
    frame_number: usize,
}

impl Vulkan {
    pub fn new(window: Arc<Window>) -> anyhow::Result<Self> {
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

        let (graphics_queue_index, _graphics_queue) = device.get_queue(QueueType::Graphics)?;

        let swapchain_builder = SwapchainBuilder::new(instance.clone(), device.clone())
            .use_default_format_selection()
            .use_default_present_modes();

        let swapchain = swapchain_builder.build()?;
        let images = swapchain.get_images()?;
        let image_views = swapchain.get_image_views()?;

        //create a command pool for commands submitted to the graphics queue.
        //we also want the pool to allow for resetting of individual command buffers
        let command_pool_info = vk::CommandPoolCreateInfo::builder()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(graphics_queue_index as _);

        let frames = (0..FRAME_OVERLAP)
            .map(|_| {
                let command_pool = unsafe {
                    device
                        .create_command_pool(&command_pool_info, None)
                        .unwrap()
                };

                // allocate the default command buffer that we will use for rendering
                let cmd_alloc_info = vk::CommandBufferAllocateInfo::builder()
                    .command_pool(command_pool)
                    .command_buffer_count(1)
                    .level(vk::CommandBufferLevel::PRIMARY);

                let command_buffer =
                    *unsafe { device.allocate_command_buffers(&cmd_alloc_info).unwrap() }
                        .first()
                        .unwrap();

                let swapchain_semaphore = unsafe {
                    device
                        .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                        .unwrap()
                };

                let render_semaphore = unsafe {
                    device
                        .create_semaphore(&vk::SemaphoreCreateInfo::default(), None)
                        .unwrap()
                };

                let render_fence = unsafe {
                    device
                        .create_fence(
                            &vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED),
                            None,
                        )
                        .unwrap()
                };

                FrameData {
                    command_pool,
                    command_buffer,
                    swapchain_semaphore,
                    render_semaphore,
                    render_fence,
                }
            })
            .collect::<Vec<FrameData>>();

        Ok(Self {
            window,
            instance,
            device,
            swapchain,
            images,
            image_views,
            frame_number: 0,
            frames,
        })
    }

    pub fn get_current_frame(&self) -> &FrameData {
        &self.frames[self.frame_number % FRAME_OVERLAP]
    }
}

impl Drop for Vulkan {
    fn drop(&mut self) {
        unsafe { self.device.device_wait_idle().unwrap() };

        for frame in self.frames.iter_mut() {
            unsafe {
                self.device
                    .free_command_buffers(frame.command_pool, &[frame.command_buffer]);
                self.device.destroy_command_pool(frame.command_pool, None);
                self.device.destroy_fence(frame.render_fence, None);
                self.device.destroy_semaphore(frame.render_semaphore, None);
                self.device
                    .destroy_semaphore(frame.swapchain_semaphore, None);
            }
        }

        // Destroy image views via the swapchain helper before destroying the swapchain/device
        self.swapchain.destroy_image_views().ok();

        // Cleanup and destroy swapchain/device/instance
        self.swapchain.destroy();
        self.device.destroy();
        self.instance.destroy();
    }
}

#[derive(Default, Debug)]
struct App {
    vulkan: Option<Vulkan>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let init_vulkan = || -> anyhow::Result<Vulkan> {
            let window = Arc::new(event_loop.create_window(WindowAttributes::default())?);

            Vulkan::new(window)
        };

        match init_vulkan() {
            Ok(vulkan) => {
                self.vulkan.replace(vulkan);
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
