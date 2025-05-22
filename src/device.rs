use std::borrow::Cow;
use crate::Instance;
use ash::vk;

#[repr(u8)]
#[derive(Default, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum PreferredDeviceType {
    Other = 0,
    Integrated = 1,
    #[default]
    Discrete = 2,
    VirtualGpu = 3,
    Cpu = 4,
}

#[derive(Default, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum Suitable {
    #[default]
    Yes,
    Partial,
    No,
}

#[derive(Default, Debug)]
pub struct PhysicalDevice {
    name: String,
    physical_device: vk::PhysicalDevice,
    surface: vk::SurfaceKHR,

    features: vk::PhysicalDeviceFeatures,
    properties: vk::PhysicalDeviceProperties,
    memory_properties: vk::PhysicalDeviceProperties,

    instance_version: u32,
    extensions_to_enable: Vec<String>,
    available_extensions: Vec<String>,
    queue_families: Vec<vk::QueueFamilyProperties>,
    defer_surface_initialization: bool,
    properties2_ext_enabled: bool,
    suitable: Suitable,
}

struct PhysicalDeviceInstanceInfo<'a> {
    instance: &'a ash::Instance,
    surface: vk::SurfaceKHR,
    version: u32,
    headless: bool,
    properties2_ext_enabled: bool,
}

#[derive(Debug)]
struct SelectionCriteria<'a> {
    name: Cow<'a, str>,
    preferred_device_type: PreferredDeviceType,
    allow_any_type: bool,
    require_present: bool,
    require_dedicated_transfer_queue: bool,
    require_dedicated_compute_queue: bool,
    require_separate_transfer_queue: bool,
    require_separate_compute_queue: bool,
    required_mem_size: vk::DeviceSize,
    required_extensions: Vec<Cow<'a, str>>,
    required_version: u32,
    required_features: vk::PhysicalDeviceFeatures,
    required_features2: vk::PhysicalDeviceFeatures2<'a>,

    // extended_features_chain
    defer_surface_initialization: bool,
    use_first_gpu_unconditionally: bool,
    enable_portability_subset: bool,
}

impl<'a> Default for SelectionCriteria<'a> {
    fn default() -> Self {
        Self {
            name: Cow::default(),
            preferred_device_type: PreferredDeviceType::Discrete,
            allow_any_type: true,
            require_present: true,
            require_dedicated_transfer_queue: false,
            require_dedicated_compute_queue: false,
            require_separate_transfer_queue: false,
            require_separate_compute_queue: false,
            required_mem_size: 0,
            required_extensions: vec![],
            required_version: vk::API_VERSION_1_0,
            required_features: vk::PhysicalDeviceFeatures::default(),
            required_features2: vk::PhysicalDeviceFeatures2::default(),
            defer_surface_initialization: false,
            use_first_gpu_unconditionally: false,
            enable_portability_subset: true,
        }
    }
}

pub struct PhysicalDeviceSelector<'a> {
    instance_info: PhysicalDeviceInstanceInfo<'a>,
    selection_criteria: SelectionCriteria<'a>,
}

impl<'a> PhysicalDeviceSelector<'a> {
    pub fn new(
        instance: &'a Instance<'a>,
        surface: Option<vk::SurfaceKHR>,
    ) -> PhysicalDeviceSelector<'a> {
        Self {
            instance_info: PhysicalDeviceInstanceInfo {
                instance: instance.as_ref(),
                surface: surface.unwrap_or_else(Default::default),
                version: instance.instance_version,
                headless: instance.headless,
                properties2_ext_enabled: instance.properties2_ext_enabled,
            },
            selection_criteria: SelectionCriteria {
                require_present: !instance.headless,
                required_version: instance.api_version,
                ..Default::default()
            },
        }
    }

    pub fn surface(mut self, surface: vk::SurfaceKHR) -> Self {
        self.instance_info.surface = surface;
        self
    }

    pub fn name(mut self, name: impl Into<Cow<'a, str>>) -> Self {
        self.selection_criteria.name = name.into();
        self
    }

    pub fn preferred_device_type(mut self, device_type: PreferredDeviceType) -> Self {
        self.selection_criteria.preferred_device_type = device_type;
        self
    }

    pub fn allow_any_gpu_device_type(mut self, allow: bool) -> Self {
        self.selection_criteria.allow_any_type = allow;
        self
    }

    pub fn require_dedicated_transfer_queue(mut self, require: bool) -> Self {
        self.selection_criteria.require_dedicated_transfer_queue = require;
        self
    }

    pub fn require_dedicated_compute_queue(mut self, require: bool) -> Self {
        self.selection_criteria.require_dedicated_compute_queue = require;
        self
    }

    pub fn require_separate_transfer_queue(mut self, require: bool) -> Self {
        self.selection_criteria.require_separate_transfer_queue = require;
        self
    }

    pub fn require_separate_compute_queue(mut self, require: bool) -> Self {
        self.selection_criteria.require_separate_compute_queue = require;
        self
    }

    pub fn required_device_memory_size(mut self, required: vk::DeviceSize) -> Self {
        self.selection_criteria.required_mem_size = required;
        self
    }

    pub fn select(self) -> crate::Result<PhysicalDevice> {
        if self.selection_criteria.require_present && !self.selection_criteria.defer_surface_initialization {
            if self.instance_info.surface == vk::SurfaceKHR::default() {
                return Err(crate::PhysicalDeviceError::NoSurfaceProvided.into())
            }
        };

        let physical_devices = unsafe { self.instance_info.instance.enumerate_physical_devices() }.map_err(|_| crate::PhysicalDeviceError::FailedToEnumeratePhysicalDevices)?;
        if physical_devices.is_empty() {
            return Err(crate::PhysicalDeviceError::NoPhysicalDevicesFound.into())
        };
        
        todo!()
    }
}
