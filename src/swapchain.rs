use crate::Device;
use crate::Instance;
use crate::device::QueueType;
use crate::error::FormatError;
use ash::vk::{AllocationCallbacks, Handle, SwapchainKHR};
use ash::{khr, vk};
use std::cell::RefCell;
use std::ops::Deref;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Ord, PartialOrd, Eq, PartialEq)]
enum Priority {
    Main,
    Fallback,
}

#[derive(Debug, Clone)]
struct Format<'a> {
    inner: vk::SurfaceFormat2KHR<'a>,
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
    swapchain_device: Arc<khr::swapchain::Device>,
    allocation_callbacks: Option<AllocationCallbacks<'static>>,
    desired_formats: Vec<Format<'static>>,
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
    surface_instance: Option<&khr::surface::Instance>,
    surface: Option<vk::SurfaceKHR>,
) -> crate::Result<SurfaceFormatDetails> {
    let Some((surface_instance, surface)) = surface_instance.zip(surface) else {
        return Err(crate::SwapchainError::SurfaceHandleNotProvided.into());
    };

    let capabilities =
        unsafe { surface_instance.get_physical_device_surface_capabilities(phys_device, surface) }?;
    let formats =
        unsafe { surface_instance.get_physical_device_surface_formats(phys_device, surface) }?;
    let present_modes = unsafe {
        surface_instance.get_physical_device_surface_present_modes(phys_device, surface)
    }?;

    Ok(SurfaceFormatDetails {
        capabilities,
        formats,
        present_modes,
    })
}

fn default_formats<'a>() -> Vec<Format<'a>> {
    vec![
        Format {
            inner: vk::SurfaceFormat2KHR::default().surface_format(
                vk::SurfaceFormatKHR::default()
                    .format(vk::Format::B8G8R8A8_SRGB)
                    .color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR),
            ),
            priority: Priority::Main,
        },
        Format {
            inner: vk::SurfaceFormat2KHR::default().surface_format(
                vk::SurfaceFormatKHR::default()
                    .format(vk::Format::R8G8B8_SRGB)
                    .color_space(vk::ColorSpaceKHR::SRGB_NONLINEAR),
            ),
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
            let mut actual_extent = vk::Extent2D::default()
                .width(self.desired_width)
                .height(self.desired_height);

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
            swapchain_device: Arc::new(khr::swapchain::Device::new(
                &instance.instance,
                device.as_ref().as_ref(),
            )),
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

    pub fn desired_format(mut self, format: vk::SurfaceFormat2KHR<'static>) -> Self {
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

    pub fn fallback_format(mut self, format: vk::SurfaceFormat2KHR<'static>) -> Self {
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

    pub fn allocation_callbacks(
        mut self,
        allocation_callbacks: AllocationCallbacks<'static>,
    ) -> Self {
        self.allocation_callbacks = Some(allocation_callbacks);
        self
    }

    /// This method should be called with previously created [`Swapchain`].
    ///
    /// # Note:
    /// This method will mark old swapchain and destroy it when creating a new one.
    pub fn set_old_swapchain(&self, swapchain: Swapchain) {
        swapchain.destroy_image_views();
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
            self.device.physical_device().physical_device,
            self.instance.surface_instance.as_ref(),
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

        let mut swapchain_create_info = vk::SwapchainCreateInfoKHR::default()
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
            self.swapchain_device
                .create_swapchain(&swapchain_create_info, self.allocation_callbacks.as_ref())
        }
        .map_err(|_| crate::SwapchainError::FailedCreateSwapchain)?;

        if old_swapchain != 0 {
            unsafe {
                self.swapchain_device.destroy_swapchain(
                    SwapchainKHR::from_raw(old_swapchain),
                    self.allocation_callbacks.as_ref(),
                )
            }
        }

        Ok(Swapchain {
            device: self.device.clone(),
            swapchain,
            extent,
            swapchain_device: self.swapchain_device.clone(),
            image_format: surface_format.format,
            image_usage_flags: self.image_usage_flags,
            instance_version: self.instance.instance_version,
            allocation_callbacks: self.allocation_callbacks,
            image_views: Mutex::new(Vec::with_capacity(image_count as _)),
        })
    }
}

pub struct Swapchain {
    device: Arc<Device>,
    swapchain: vk::SwapchainKHR,
    swapchain_device: Arc<khr::swapchain::Device>,
    pub image_format: vk::Format,
    pub extent: vk::Extent2D,
    image_usage_flags: vk::ImageUsageFlags,
    instance_version: u32,
    allocation_callbacks: Option<AllocationCallbacks<'static>>,
    image_views: Mutex<Vec<vk::ImageView>>,
}

impl Swapchain {
    pub fn get_images(&self) -> crate::Result<Vec<vk::Image>> {
        let images = unsafe { self.swapchain_device.get_swapchain_images(self.swapchain) }?;

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
            vk::ImageViewUsageCreateInfo::default().usage(self.image_usage_flags);

        let views: Vec<_> = images
            .into_iter()
            .map(|image| {
                let mut create_info = vk::ImageViewCreateInfo::default();

                if self.instance_version >= vk::API_VERSION_1_1 {
                    create_info = create_info.push_next(&mut desired_flags);
                }

                create_info.image = image;
                create_info.view_type = vk::ImageViewType::TYPE_2D;
                create_info.format = self.image_format;
                create_info.components.r = vk::ComponentSwizzle::IDENTITY;
                create_info.components.g = vk::ComponentSwizzle::IDENTITY;
                create_info.components.b = vk::ComponentSwizzle::IDENTITY;
                create_info.components.a = vk::ComponentSwizzle::IDENTITY;
                create_info.subresource_range.aspect_mask = vk::ImageAspectFlags::COLOR;
                create_info.subresource_range.base_mip_level = 0;
                create_info.subresource_range.level_count = 1;
                create_info.subresource_range.base_array_layer = 0;
                create_info.subresource_range.layer_count = 1;

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
            self.swapchain_device
                .destroy_swapchain(self.swapchain, self.allocation_callbacks.as_ref())
        };
    }
}

impl AsRef<SwapchainKHR> for Swapchain {
    fn as_ref(&self) -> &SwapchainKHR {
        &self.swapchain
    }
}

impl Deref for Swapchain {
    type Target = khr::swapchain::Device;

    fn deref(&self) -> &Self::Target {
        &*self.swapchain_device
    }
}
