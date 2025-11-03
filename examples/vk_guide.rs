use std::sync::Arc;
use std::time::Duration;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt};
use vulkanalia::vk::{
    DeviceV1_0, DeviceV1_3, Handle, HasBuilder, KhrSwapchainExtensionDeviceCommands,
};
use vulkanalia::{Version, vk};
use vulkanalia_bootstrap::{
    Device, DeviceBuilder, Instance, InstanceBuilder, PhysicalDeviceSelector, PreferredDeviceType,
    QueueType, Swapchain, SwapchainBuilder,
};
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{Window, WindowAttributes, WindowId};

#[derive(Debug)]
struct FrameData {
    command_pool: vk::CommandPool,
    command_buffer: vk::CommandBuffer,
    swapchain_semaphore: vk::Semaphore,
    render_semaphore: vk::Semaphore,
    render_fence: vk::Fence,
}

#[derive(Debug)]
struct VulkanEngine {
    window: Arc<Window>,
    instance: Arc<Instance>,
    device: Arc<Device>,
    swapchain: Swapchain,
    swapchain_images: Vec<vk::Image>,
    swapchain_image_views: Vec<vk::ImageView>,
    graphics_queue: vk::Queue,

    frames: Vec<FrameData>,
    frame_number: usize,
}

impl VulkanEngine {
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

        let (graphics_queue_index, graphics_queue) = device.get_queue(QueueType::Graphics)?;

        let window_extent = window.inner_size();

        let swapchain_builder = SwapchainBuilder::new(instance.clone(), device.clone())
            .desired_format(
                vk::SurfaceFormat2KHR::builder()
                    .surface_format(
                        vk::SurfaceFormatKHR::builder()
                            .format(vk::Format::B8G8R8A8_UNORM)
                            .color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR)
                            .build(),
                    )
                    .build(),
            )
            .add_image_usage_flags(vk::ImageUsageFlags::TRANSFER_DST)
            .desired_size(
                vk::Extent2D::builder()
                    .width(window_extent.width)
                    .height(window_extent.height)
                    .build(),
            )
            .use_default_present_modes();

        let swapchain = swapchain_builder.build()?;
        let swapchain_images = swapchain.get_images()?;
        let swapchain_image_views = swapchain.get_image_views()?;
        let frame_overlap = swapchain_images.len();

        //create a command pool for commands submitted to the graphics queue.
        //we also want the pool to allow for resetting of individual command buffers
        let command_pool_info = vk::CommandPoolCreateInfo::builder()
            .flags(vk::CommandPoolCreateFlags::RESET_COMMAND_BUFFER)
            .queue_family_index(graphics_queue_index as _);

        let frames = (0..frame_overlap)
            .map(|_| {
                unsafe {
                    let command_pool = device.create_command_pool(&command_pool_info, None)?;

                    // allocate the default command buffer that we will use for rendering
                    let cmd_alloc_info = vk::CommandBufferAllocateInfo::builder()
                        .command_pool(command_pool)
                        .command_buffer_count(1)
                        .level(vk::CommandBufferLevel::PRIMARY);

                    let command_buffer = *device
                        .allocate_command_buffers(&cmd_alloc_info)?
                        .first()
                        .ok_or(anyhow::anyhow!("No command buffer allocated"))?;

                    let swapchain_semaphore =
                        device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?;

                    let render_semaphore =
                        device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)?;

                    let render_fence = device.create_fence(
                        &vk::FenceCreateInfo::builder().flags(vk::FenceCreateFlags::SIGNALED),
                        None,
                    )?;

                    Ok(FrameData {
                        command_pool,
                        command_buffer,
                        swapchain_semaphore,
                        render_semaphore,
                        render_fence,
                    })
                }
            })
            .collect::<anyhow::Result<Vec<FrameData>>>()?;

        Ok(Self {
            window,
            instance,
            device,
            swapchain,
            swapchain_images,
            swapchain_image_views,
            graphics_queue,
            frame_number: 0,
            frames,
        })
    }

    pub fn draw(&mut self) -> anyhow::Result<()> {
        let current_frame = self.get_current_frame();

        unsafe {
            self.device.wait_for_fences(
                &[current_frame.render_fence],
                true,
                Duration::from_secs(1).as_nanos() as _,
            )?;

            self.device.reset_fences(&[current_frame.render_fence])?;

            let (swapchain_image_index, err) = self.device.acquire_next_image_khr(
                *self.swapchain.as_ref(),
                // use a large timeout to avoid spurious TIMEOUT results on some platforms
                u64::MAX,
                current_frame.swapchain_semaphore,
                vk::Fence::null(),
            )?;

            // Handle common success codes: SUCCESS and SUBOPTIMAL are fine.
            // If we receive a TIMEOUT, skip this frame instead of failing the whole render loop.
            if matches!(err, vk::SuccessCode::TIMEOUT) {
                eprintln!("acquire_next_image_khr timed out, skipping frame");
                return Ok(());
            }

            if !matches!(
                err,
                vk::SuccessCode::SUCCESS | vk::SuccessCode::SUBOPTIMAL_KHR
            ) {
                return Err(anyhow::anyhow!(
                    "Failed acquiring next swapchain image, {}",
                    err
                ));
            }

            let current_image = self.swapchain_images[swapchain_image_index as usize];
            let cmd = current_frame.command_buffer;

            self.device
                .reset_command_buffer(cmd, vk::CommandBufferResetFlags::empty())?;

            self.device.begin_command_buffer(
                cmd,
                &vk::CommandBufferBeginInfo::builder()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT),
            )?;

            //make the swapchain image into writeable mode before rendering
            transition_image(
                self.device.clone(),
                cmd,
                current_image,
                vk::ImageLayout::UNDEFINED,
                vk::ImageLayout::GENERAL,
            );

            let flash = (self.frame_number as f32 / 120f32).sin().abs();
            let clear_value = vk::ClearColorValue {
                float32: { [0.0f32, 0.0f32, flash, 1.0f32] },
            };

            let clear_range = image_subresource_range(vk::ImageAspectFlags::COLOR);

            self.device.cmd_clear_color_image(
                cmd,
                current_image,
                vk::ImageLayout::GENERAL,
                &clear_value,
                &[clear_range],
            );

            // Make the swapchain image into presentable mode
            transition_image(
                self.device.clone(),
                cmd,
                current_image,
                vk::ImageLayout::GENERAL,
                vk::ImageLayout::PRESENT_SRC_KHR,
            );

            // Finalize the command buffer (we can no longer add commands, but it can now be executed)
            self.device.end_command_buffer(cmd)?;

            //prepare the submission to the queue.
            //we want to wait on the _presentSemaphore, as that semaphore is signaled when the swapchain is ready
            //we will signal the _renderSemaphore, to signal that rendering has finished

            let cmd_info = [vk::CommandBufferSubmitInfo::builder().command_buffer(cmd)];

            let wait_info = [vk::SemaphoreSubmitInfo::builder()
                .semaphore(current_frame.swapchain_semaphore)
                .stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                .value(1)];
            let signal_info = [vk::SemaphoreSubmitInfo::builder()
                .semaphore(current_frame.render_semaphore)
                .stage_mask(vk::PipelineStageFlags2::ALL_GRAPHICS)
                .value(1)];

            let submit_info = vk::SubmitInfo2::builder()
                .command_buffer_infos(&cmd_info)
                .signal_semaphore_infos(&signal_info)
                .wait_semaphore_infos(&wait_info);

            //submit command buffer to the queue and execute it.
            // _renderFence will now block until the graphic commands finish execution
            self.device.queue_submit2(
                self.graphics_queue,
                &[submit_info],
                current_frame.render_fence,
            )?;

            // Present the image back to the swapchain so it becomes available for acquisition again.
            let wait_semaphores = [current_frame.render_semaphore];
            let swapchains = [*self.swapchain.as_ref()];
            let image_indices = [swapchain_image_index];

            let present_info = vk::PresentInfoKHR::builder()
                .wait_semaphores(&wait_semaphores)
                .swapchains(&swapchains)
                .image_indices(&image_indices);

            // queue_present_khr is provided by the swapchain extension trait.
            self.device
                .queue_present_khr(self.graphics_queue, &present_info)?;
        }

        self.frame_number += 1;

        Ok(())
    }

    fn get_current_frame(&self) -> &FrameData {
        &self.frames[self.frame_number % self.swapchain_images.len()]
    }
}

impl Drop for VulkanEngine {
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
    vulkan_engine: Option<VulkanEngine>,
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let init_vulkan = || -> anyhow::Result<VulkanEngine> {
            let window = Arc::new(event_loop.create_window(WindowAttributes::default())?);

            VulkanEngine::new(window)
        };

        match init_vulkan() {
            Ok(vulkan) => {
                self.vulkan_engine.replace(vulkan);
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
            WindowEvent::RedrawRequested => {
                let vulkan_engine = self.vulkan_engine.as_mut().unwrap();
                vulkan_engine.draw().unwrap();
                vulkan_engine.window.request_redraw();
            }
            _ => (),
        }
    }
}

fn image_subresource_range(aspect_mask: vk::ImageAspectFlags) -> vk::ImageSubresourceRange {
    vk::ImageSubresourceRange::builder()
        .aspect_mask(aspect_mask)
        .base_mip_level(0)
        .level_count(vk::REMAINING_MIP_LEVELS)
        .base_array_layer(0)
        .layer_count(vk::REMAINING_ARRAY_LAYERS)
        .build()
}

fn transition_image(
    device: Arc<Device>,
    cmd: vk::CommandBuffer,
    image: vk::Image,
    current_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) {
    let aspect_mask = if new_layout == vk::ImageLayout::DEPTH_ATTACHMENT_OPTIMAL {
        vk::ImageAspectFlags::DEPTH
    } else {
        vk::ImageAspectFlags::COLOR
    };

    let image_barriers = [vk::ImageMemoryBarrier2::builder()
        .src_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
        .src_access_mask(vk::AccessFlags2::MEMORY_WRITE)
        .dst_stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
        .dst_access_mask(vk::AccessFlags2::MEMORY_READ | vk::AccessFlags2::MEMORY_WRITE)
        .old_layout(current_layout)
        .new_layout(new_layout)
        .subresource_range(image_subresource_range(aspect_mask))
        .image(image)];

    let dep_info = vk::DependencyInfo::builder().image_memory_barriers(&image_barriers);

    unsafe {
        device.cmd_pipeline_barrier2(cmd, &dep_info);
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
