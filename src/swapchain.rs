use crate::Device;
use crate::Instance;
use crate::device::QueueType;
use crate::error::FormatError;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use vulkanalia::Version;
use vulkanalia::vk;
use vulkanalia::vk::DeviceV1_0;
use vulkanalia::vk::HasBuilder;
use vulkanalia::vk::KhrSurfaceExtensionInstanceCommands;
use vulkanalia::vk::KhrSwapchainExtensionDeviceCommands;
use vulkanalia::vk::{AllocationCallbacks, Handle, SwapchainKHR};

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
enum Priority {
    Main,
    Fallback,
}

#[derive(Debug, Clone)]
struct Format {
    inner: vk::SurfaceFormat2KHR,
    priority: Priority,
}

#[derive(Debug, Clone)]
struct PresentMode {
    inner: vk::PresentModeKHR,
    priority: Priority,
}

pub struct SwapchainBuilder {
    instance: Arc<Instance>,
    device: Arc<Device>,
    allocation_callbacks: Option<AllocationCallbacks>,
    desired_formats: Vec<Format>,
    create_flags: vk::SwapchainCreateFlagsKHR,
    desired_width: u32,
    desired_height: u32,
    array_layer_count: u32,
    min_image_count: u32,
    required_min_image_count: u32,
    image_usage_flags: vk::ImageUsageFlags,
    composite_alpha_flags_khr: vk::CompositeAlphaFlagsKHR,
    desired_present_modes: Vec<PresentMode>,
    pre_transform: vk::SurfaceTransformFlagsKHR,
    clipped: bool,
    old_swapchain: AtomicU64,
    graphics_queue_index: usize,
    present_queue_index: usize,
}

struct SurfaceFormatDetails {
    capabilities: vk::SurfaceCapabilitiesKHR,
    formats: Vec<vk::SurfaceFormatKHR>,
    present_modes: Vec<vk::PresentModeKHR>,
}

fn query_surface_support_details(
    phys_device: vk::PhysicalDevice,
    instance: &vulkanalia::Instance,
    surface: Option<vk::SurfaceKHR>,
) -> crate::Result<SurfaceFormatDetails> {
    let Some(surface) = surface else {
        return Err(crate::SwapchainError::SurfaceHandleNotProvided.into());
    };

    let capabilities =
        unsafe { instance.get_physical_device_surface_capabilities_khr(phys_device, surface) }?;
    let formats =
        unsafe { instance.get_physical_device_surface_formats_khr(phys_device, surface) }?;
    let present_modes =
        unsafe { instance.get_physical_device_surface_present_modes_khr(phys_device, surface) }?;

    Ok(SurfaceFormatDetails {
        capabilities,
        formats,
        present_modes,
    })
}

fn default_formats<'a>() -> Vec<Format> {
    vec![
        Format {
            inner: vk::SurfaceFormat2KHR {
                surface_format: vk::SurfaceFormatKHR {
                    format: vk::Format::B8G8R8A8_SRGB,
                    color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
                    ..Default::default()
                },
                ..Default::default()
            },
            priority: Priority::Main,
        },
        Format {
            inner: vk::SurfaceFormat2KHR {
                surface_format: vk::SurfaceFormatKHR {
                    format: vk::Format::R8G8B8_SRGB,
                    color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
                    ..Default::default()
                },
                ..Default::default()
            },
            priority: Priority::Fallback,
        },
    ]
}

fn default_present_modes() -> Vec<PresentMode> {
    vec![
        PresentMode {
            inner: vk::PresentModeKHR::MAILBOX,
            priority: Priority::Main,
        },
        PresentMode {
            inner: vk::PresentModeKHR::FIFO,
            priority: Priority::Fallback,
        },
    ]
}

fn find_desired_surface_format(
    available: &[vk::SurfaceFormatKHR],
    desired: &mut [Format],
) -> crate::Result<vk::SurfaceFormatKHR> {
    if !desired.is_sorted_by_key(|f| f.priority.clone()) {
        desired.sort_unstable_by_key(|f| f.priority.clone());
    }

    for desired in desired.iter() {
        for available in available {
            if desired.inner.surface_format.format == available.format
                && desired.inner.surface_format.color_space == available.color_space
            {
                return Ok(desired.inner.surface_format);
            }
        }
    }

    Err(crate::SwapchainError::NoSuitableDesiredFormat(FormatError {
        available: available.to_vec(),
        desired: desired.iter().map(|d| d.inner.surface_format).collect(),
    })
    .into())
}

fn find_best_surface_format(
    available: &[vk::SurfaceFormatKHR],
    desired: &mut [Format],
) -> vk::SurfaceFormatKHR {
    find_desired_surface_format(available, desired).unwrap_or(available[0])
}

fn find_present_mode(
    available: &[vk::PresentModeKHR],
    desired: &mut [PresentMode],
) -> vk::PresentModeKHR {
    if !desired.is_sorted_by_key(|f| f.priority.clone()) {
        desired.sort_unstable_by_key(|f| f.priority.clone());
    }

    for desired in desired {
        for available in available {
            if &desired.inner == available {
                return *available;
            }
        }
    }

    vk::PresentModeKHR::FIFO
}

impl SwapchainBuilder {
    fn find_extent(&self, capabilities: &vk::SurfaceCapabilitiesKHR) -> vk::Extent2D {
        if capabilities.current_extent.width != u32::MAX {
            capabilities.current_extent
        } else {
            let mut actual_extent = vk::Extent2D {
                width: self.desired_width,
                height: self.desired_height,
            };

            actual_extent.width = capabilities
                .min_image_extent
                .width
                .max(capabilities.max_image_extent.width.min(actual_extent.width));
            actual_extent.height = capabilities.min_image_extent.height.max(
                capabilities
                    .max_image_extent
                    .height
                    .min(actual_extent.height),
            );

            actual_extent
        }
    }

    pub fn new(instance: Arc<Instance>, device: Arc<Device>) -> Self {
        Self {
            graphics_queue_index: device.get_queue(QueueType::Graphics).unwrap().0,
            present_queue_index: device.get_queue(QueueType::Present).unwrap().0,
            instance,
            device,
            allocation_callbacks: None,
            desired_formats: Vec::with_capacity(4),
            create_flags: vk::SwapchainCreateFlagsKHR::default(),
            desired_width: 256,
            desired_height: 256,
            array_layer_count: 1,
            min_image_count: 0,
            required_min_image_count: 0,
            image_usage_flags: vk::ImageUsageFlags::COLOR_ATTACHMENT,
            pre_transform: vk::SurfaceTransformFlagsKHR::default(),
            desired_present_modes: Vec::with_capacity(4),
            composite_alpha_flags_khr: vk::CompositeAlphaFlagsKHR::OPAQUE,
            clipped: true,
            old_swapchain: Default::default(),
        }
    }

    pub fn desired_format(mut self, format: vk::SurfaceFormat2KHR) -> Self {
        self.desired_formats.push(Format {
            inner: format,
            priority: Priority::Main,
        });
        self
    }

    pub fn desired_size(mut self, size: vk::Extent2D) -> Self {
        self.desired_width = size.width;
        self.desired_height = size.height;
        self
    }

    pub fn fallback_format(mut self, format: vk::SurfaceFormat2KHR) -> Self {
        self.desired_formats.push(Format {
            inner: format,
            priority: Priority::Fallback,
        });
        self
    }

    /// Use the default swapchain formats. This is done if no formats are provided.
    ///
    /// Default surface format is [
    ///     [`vk::Format::B8G8R8A8_SRGB`],
    ///     [`vk::ColorSpaceKHR::SRGB_NONLINEAR`]
    /// ]
    pub fn use_default_format_selection(mut self) -> Self {
        self.desired_formats = default_formats();
        self
    }

    pub fn desired_present_mode(mut self, present_mode: vk::PresentModeKHR) -> Self {
        self.desired_present_modes.push(PresentMode {
            inner: present_mode,
            priority: Priority::Main,
        });
        self
    }

    pub fn fallback_present_mode(mut self, present_mode: vk::PresentModeKHR) -> Self {
        self.desired_present_modes.push(PresentMode {
            inner: present_mode,
            priority: Priority::Fallback,
        });
        self
    }

    pub fn use_default_present_modes(mut self) -> Self {
        self.desired_present_modes = default_present_modes();
        self
    }

    /// Sets the desired minimum image count for the swapchain.
    /// Note that the presentation engine is always free to create more images than requested.
    /// You may pass one of the values specified in the BufferMode enum, or any integer value.
    /// For instance, if you pass DOUBLE_BUFFERING, the presentation engine is allowed to give you a double buffering setup,
    /// triple buffering, or more. This is up to the drivers.
    pub fn desired_min_image_count(mut self, min_image_count: u32) -> Self {
        self.min_image_count = min_image_count;
        self
    }

    /// Set whether the Vulkan implementation is allowed to discard rendering operations that
    /// affect regions of the surface that are not visible. Default is true.
    /// # Note:
    /// Applications should use the default of true if:
    /// - They do not expect to read back the content of presentable images before presenting them or after reacquiring them
    /// - If their fragment shaders do not have any side effects that require them to run for all pixels in the presentable image.
    pub fn clipped(mut self, clipped: bool) -> Self {
        self.clipped = clipped;
        self
    }

    pub fn create_flags(mut self, flags: vk::SwapchainCreateFlagsKHR) -> Self {
        self.create_flags = flags;
        self
    }

    /// Set the bitmask of the image usage for acquired swapchain images.
    /// If the surface capabilities cannot allow it, building the swapchain will result in the `SwapchainError::required_usage_not_supported` error.
    pub fn image_usage_flags(mut self, flags: vk::ImageUsageFlags) -> Self {
        self.image_usage_flags = flags;
        self
    }

    /// Add a image usage to the bitmask for acquired swapchain images.
    pub fn add_image_usage_flags(mut self, flags: vk::ImageUsageFlags) -> Self {
        self.image_usage_flags |= flags;
        self
    }

    pub fn allocation_callbacks(mut self, allocation_callbacks: AllocationCallbacks) -> Self {
        self.allocation_callbacks = Some(allocation_callbacks);
        self
    }

    /// This method should be called with previously created [`Swapchain`].
    ///
    /// # Note:
    /// This method will mark old swapchain and destroy it when creating a new one.
    pub fn set_old_swapchain(&self, swapchain: Swapchain) {
        if swapchain.destroy_image_views().is_err() {
            #[cfg(feature = "enable_tracing")]
            tracing::warn!("Could not destroy swapchain image views");
            return;
        };
        self.old_swapchain
            .store(swapchain.swapchain.as_raw(), Ordering::Relaxed);
    }

    pub fn build(&self) -> crate::Result<Swapchain> {
        if self.instance.surface.is_none() {
            return Err(crate::SwapchainError::SurfaceHandleNotProvided.into());
        };

        let mut desired_formats = self.desired_formats.clone();
        if desired_formats.is_empty() {
            desired_formats = default_formats();
        };

        let mut desired_present_modes = self.desired_present_modes.clone();
        if desired_present_modes.is_empty() {
            desired_present_modes = default_present_modes();
        }

        let surface_support = query_surface_support_details(
            *self.device.physical_device().as_ref(),
            &self.instance.instance,
            self.instance.surface,
        )?;

        let mut image_count = self.min_image_count;
        if image_count >= 1 {
            if self.required_min_image_count < surface_support.capabilities.min_image_count {
                return Err(crate::SwapchainError::RequiredMinImageCountTooLow.into());
            }

            image_count = surface_support.capabilities.min_image_count;
        } else if image_count == 0 {
            // We intentionally use minImageCount + 1 to maintain existing behavior,
            // even if it typically results in triple buffering on most systems.
            image_count = surface_support.capabilities.min_image_count + 1;
        } else if image_count < surface_support.capabilities.min_image_count {
            image_count = surface_support.capabilities.min_image_count
        }

        if surface_support.capabilities.max_image_count > 0
            && image_count > surface_support.capabilities.max_image_count
        {
            image_count = surface_support.capabilities.max_image_count;
        }

        let surface_format =
            find_best_surface_format(&surface_support.formats, &mut desired_formats);

        let extent = self.find_extent(&surface_support.capabilities);

        let mut image_array_layers = self.array_layer_count;
        if surface_support.capabilities.max_image_array_layers < image_array_layers {
            image_array_layers = surface_support.capabilities.max_image_array_layers;
        }
        if image_array_layers == 0 {
            image_array_layers = 1;
        }

        let present_mode =
            find_present_mode(&surface_support.present_modes, &mut desired_present_modes);

        let is_unextended_present_mode =
            matches!(
                present_mode,
                |vk::PresentModeKHR::IMMEDIATE| vk::PresentModeKHR::MAILBOX
                    | vk::PresentModeKHR::FIFO
                    | vk::PresentModeKHR::FIFO_RELAXED
            );

        if is_unextended_present_mode
            && !surface_support
                .capabilities
                .supported_usage_flags
                .contains(self.image_usage_flags)
        {
            return Err(crate::SwapchainError::RequiredUsageNotSupported.into());
        };

        let mut pre_transform = self.pre_transform;
        if pre_transform == vk::SurfaceTransformFlagsKHR::default() {
            pre_transform = surface_support.capabilities.current_transform;
        }

        let old_swapchain = self.old_swapchain.load(Ordering::Relaxed);

        let mut swapchain_create_info = vk::SwapchainCreateInfoKHR::builder()
            .flags(self.create_flags)
            .surface(self.instance.surface.unwrap())
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent)
            .image_array_layers(image_array_layers)
            .image_usage(self.image_usage_flags)
            .pre_transform(pre_transform)
            .composite_alpha(self.composite_alpha_flags_khr)
            .present_mode(present_mode)
            .clipped(self.clipped)
            .old_swapchain(SwapchainKHR::from_raw(old_swapchain));

        let queue_family_indices = [
            self.graphics_queue_index as _,
            self.present_queue_index as _,
        ];

        if self.graphics_queue_index != self.present_queue_index {
            swapchain_create_info.image_sharing_mode = vk::SharingMode::CONCURRENT;
            swapchain_create_info =
                swapchain_create_info.queue_family_indices(&queue_family_indices);
        } else {
            swapchain_create_info.image_sharing_mode = vk::SharingMode::EXCLUSIVE;
        }

        let swapchain = unsafe {
            self.device
                .create_swapchain_khr(&swapchain_create_info, self.allocation_callbacks.as_ref())
        }
        .map_err(|_| crate::SwapchainError::FailedCreateSwapchain)?;

        if old_swapchain != 0 {
            unsafe {
                self.device.destroy_swapchain_khr(
                    SwapchainKHR::from_raw(old_swapchain),
                    self.allocation_callbacks.as_ref(),
                )
            }
        }

        Ok(Swapchain {
            device: self.device.clone(),
            swapchain,
            extent,
            image_format: surface_format.format,
            image_usage_flags: self.image_usage_flags,
            instance_version: self.instance.instance_version,
            allocation_callbacks: self.allocation_callbacks,
            image_views: Mutex::new(Vec::with_capacity(image_count as _)),
        })
    }
}

#[derive(Debug)]
pub struct Swapchain {
    device: Arc<Device>,
    swapchain: vk::SwapchainKHR,
    pub image_format: vk::Format,
    pub extent: vk::Extent2D,
    image_usage_flags: vk::ImageUsageFlags,
    instance_version: Version,
    allocation_callbacks: Option<AllocationCallbacks>,
    image_views: Mutex<Vec<vk::ImageView>>,
}

impl Swapchain {
    pub fn get_images(&self) -> crate::Result<Vec<vk::Image>> {
        let images = unsafe { self.device.get_swapchain_images_khr(self.swapchain) }?;

        Ok(images)
    }

    pub fn destroy_image_views(&self) -> crate::Result<()> {
        let mut image_views = self.image_views.lock().unwrap();

        for image_view in image_views.drain(..) {
            unsafe {
                self.device
                    .device()
                    .destroy_image_view(image_view, self.allocation_callbacks.as_ref())
            }
        }

        Ok(())
    }

    pub fn get_image_views(&self) -> crate::Result<Vec<vk::ImageView>> {
        let images = self.get_images()?;

        let mut desired_flags =
            vk::ImageViewUsageCreateInfo::builder().usage(self.image_usage_flags);

        let views: Vec<_> = images
            .into_iter()
            .map(|image| {
                // Build the ImageViewCreateInfo using chaining so values are actually set.
                let mut create_info = vk::ImageViewCreateInfo::builder();

                if self.instance_version >= Version::V1_1_0 {
                    create_info = create_info.push_next(&mut desired_flags);
                }

                let create_info = create_info
                    .image(image)
                    .view_type(vk::ImageViewType::_2D)
                    .format(self.image_format)
                    .components(vk::ComponentMapping::default())
                    .subresource_range(
                        vk::ImageSubresourceRange::builder()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .level_count(1)
                            .layer_count(1),
                    );

                unsafe {
                    self.device
                        .device()
                        .create_image_view(&create_info, self.allocation_callbacks.as_ref())
                }
                .map_err(Into::into)
            })
            .collect::<crate::Result<_>>()?;

        {
            let mut image_views = self.image_views.lock().unwrap();
            *image_views = views.clone();
        }

        Ok(views)
    }

    pub fn destroy(&self) {
        unsafe {
            self.device
                .destroy_swapchain_khr(self.swapchain, self.allocation_callbacks.as_ref())
        };
    }
}

impl AsRef<SwapchainKHR> for Swapchain {
    fn as_ref(&self) -> &SwapchainKHR {
        &self.swapchain
    }
}
