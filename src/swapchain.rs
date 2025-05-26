use crate::error::FormatError;
use crate::Device;
use crate::Instance;
use ash::vk::AllocationCallbacks;
use ash::{khr, vk};

#[repr(u8)]
#[derive(Debug, Clone, PartialOrd, PartialEq, Ord, Eq)]
enum BufferMode {
    SingleBuffering = 1,
    DoubleBuffering = 2,
    TripleBuffering = 3,
}

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

pub struct SwapchainBuilder<'a> {
    instance: &'a Instance<'a>,
    device: &'a Device<'a>,
    allocation_callbacks: Option<AllocationCallbacks<'a>>,
    desired_formats: Vec<Format<'a>>,
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
    old_swapchain: vk::SwapchainKHR,
}

struct SurfaceFormatDetails {
    capabilities: vk::SurfaceCapabilitiesKHR,
    formats: Vec<vk::SurfaceFormatKHR>,
    present_modes: Vec<vk::PresentModeKHR>,
}

fn query_surface_support_details<'a>(
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

impl<'a> SwapchainBuilder<'a> {
    pub fn new(instance: &'a Instance<'a>, device: &'a Device<'a>) -> Self {
        Self {
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

    pub fn desired_format(mut self, format: vk::SurfaceFormat2KHR<'a>) -> Self {
        self.desired_formats.push(Format {
            inner: format,
            priority: Priority::Main,
        });
        self
    }

    pub fn fallback_format(mut self, format: vk::SurfaceFormat2KHR<'a>) -> Self {
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

    pub fn allocation_callbacks(mut self, allocation_callbacks: AllocationCallbacks<'a>) -> Self {
        self.allocation_callbacks = Some(allocation_callbacks);
        self
    }

    pub fn build(&mut self) -> crate::Result<Swapchain<'a>> {
        if self.instance.surface.is_none() {
            return Err(crate::SwapchainError::SurfaceHandleNotProvided.into());
        };

        if self.desired_formats.is_empty() {
            self.desired_formats = default_formats();
        };
        let desired_formats = &self.desired_formats;

        if self.desired_present_modes.is_empty() {
            self.desired_present_modes = default_present_modes();
        }
        let desired_present_modes = &self.desired_present_modes;

        let surface_support = query_surface_support_details(
            self.device.physical_device(),
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
        } else {
            if image_count < surface_support.capabilities.min_image_count {
                image_count = surface_support.capabilities.min_image_count
            }
        }

        if surface_support.capabilities.max_image_count > 0 && image_count > surface_support.capabilities.max_image_count {
            image_count = surface_support.capabilities.max_image_count;
        }

        let surface_format =
            find_best_surface_format(&surface_support.formats, &mut self.desired_formats);

        todo!()
    }
}

pub struct Swapchain<'a> {
    device: vk::Device,
    swapchain: vk::SwapchainKHR,
    swapchain_device: khr::swapchain::Device,
    image_count: usize,
    image_format: vk::Format,
    color_space: vk::ColorSpaceKHR,
    image_usage_flags: vk::ImageUsageFlags,
    extent: vk::Extent2D,
    requested_min_image_count: usize,
    present_mode: vk::PresentModeKHR,
    instance_version: u32,
    allocation_callbacks: Option<AllocationCallbacks<'a>>,
}
