use crate::Instance;
use ash::{khr, vk};
use std::borrow::Cow;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};

fn supports_features(
    supported: &vk::PhysicalDeviceFeatures,
    requested: &vk::PhysicalDeviceFeatures,
) -> bool {
    macro_rules! check_feature {
        ($feature: ident) => {
            if requested.$feature == vk::TRUE && supported.$feature == vk::FALSE {
                return false;
            }
        };
    }

    check_feature!(robust_buffer_access);
    check_feature!(full_draw_index_uint32);
    check_feature!(image_cube_array);
    check_feature!(independent_blend);
    check_feature!(geometry_shader);
    check_feature!(tessellation_shader);
    check_feature!(sample_rate_shading);
    check_feature!(dual_src_blend);
    check_feature!(logic_op);
    check_feature!(multi_draw_indirect);
    check_feature!(draw_indirect_first_instance);
    check_feature!(depth_clamp);
    check_feature!(depth_bias_clamp);
    check_feature!(fill_mode_non_solid);
    check_feature!(depth_bounds);
    check_feature!(wide_lines);
    check_feature!(large_points);
    check_feature!(alpha_to_one);
    check_feature!(multi_viewport);
    check_feature!(sampler_anisotropy);
    check_feature!(texture_compression_etc2);
    check_feature!(texture_compression_astc_ldr);
    check_feature!(texture_compression_bc);
    check_feature!(occlusion_query_precise);
    check_feature!(pipeline_statistics_query);
    check_feature!(vertex_pipeline_stores_and_atomics);
    check_feature!(fragment_stores_and_atomics);
    check_feature!(shader_tessellation_and_geometry_point_size);
    check_feature!(shader_image_gather_extended);
    check_feature!(shader_storage_image_extended_formats);
    check_feature!(shader_storage_image_multisample);
    check_feature!(shader_storage_image_read_without_format);
    check_feature!(shader_storage_image_write_without_format);
    check_feature!(shader_uniform_buffer_array_dynamic_indexing);
    check_feature!(shader_sampled_image_array_dynamic_indexing);
    check_feature!(shader_storage_buffer_array_dynamic_indexing);
    check_feature!(shader_storage_image_array_dynamic_indexing);
    check_feature!(shader_clip_distance);
    check_feature!(shader_cull_distance);
    check_feature!(shader_float64);
    check_feature!(shader_int64);
    check_feature!(shader_int16);
    check_feature!(shader_resource_residency);
    check_feature!(shader_resource_min_lod);
    check_feature!(sparse_binding);
    check_feature!(sparse_residency_buffer);
    check_feature!(sparse_residency_image2_d);
    check_feature!(sparse_residency_image3_d);
    check_feature!(sparse_residency2_samples);
    check_feature!(sparse_residency4_samples);
    check_feature!(sparse_residency8_samples);
    check_feature!(sparse_residency16_samples);
    check_feature!(sparse_residency_aliased);
    check_feature!(variable_multisample_rate);
    check_feature!(inherited_queries);

    true
}

#[inline]
fn get_first_queue_index(
    families: &[vk::QueueFamilyProperties],
    desired_flags: vk::QueueFlags,
) -> Option<usize> {
    families
        .iter()
        .position(|f| f.queue_flags.contains(desired_flags))
}

/// Finds the queue which is separate from the graphics queue and has the desired flag and not the
/// undesired flag, but will select it if no better options are available for compute support. Returns
/// QUEUE_INDEX_MAX_VALUE if none is found.
fn get_separate_queue_index(
    families: &[vk::QueueFamilyProperties],
    desired_flags: vk::QueueFlags,
    undesired_flags: vk::QueueFlags,
) -> Option<usize> {
    let mut index = None;
    for (i, family) in families.iter().enumerate() {
        if family.queue_flags.contains(desired_flags)
            && !family.queue_flags.contains(vk::QueueFlags::GRAPHICS)
        {
            if !family.queue_flags.contains(undesired_flags) {
                return Some(i);
            } else {
                index = Some(i);
            }
        }
    }

    index
}

/// finds the first queue which supports only the desired flag (not graphics or transfer). Returns QUEUE_INDEX_MAX_VALUE if none is found.
fn get_dedicated_queue_index(
    families: &[vk::QueueFamilyProperties],
    desired_flags: vk::QueueFlags,
    undesired_flags: vk::QueueFlags,
) -> Option<usize> {
    families
        .iter()
        .position(|f| {
            f.queue_flags.contains(desired_flags)
                && !f.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                && !f.queue_flags.contains(undesired_flags)
        })
}

fn get_present_queue_index(
    instance: Option<&khr::surface::Instance>,
    device: &vk::PhysicalDevice,
    surface: Option<vk::SurfaceKHR>,
    families: &[vk::QueueFamilyProperties],
) -> Option<usize> {
    for (i, _) in families.iter().enumerate() {
        if let Some((surface, instance)) = surface.zip(instance) {
            let present_support = unsafe { instance.get_physical_device_surface_support(*device, i as u32, surface) };

            if let Ok(present_support) = present_support {
                if present_support {
                    return Some(i);
                }
            }
        }
    };

    None
}

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
    surface: Option<vk::SurfaceKHR>,

    features: vk::PhysicalDeviceFeatures,
    properties: vk::PhysicalDeviceProperties,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
    instance_version: u32,
    extensions_to_enable: HashSet<String>,
    available_extensions: HashSet<String>,
    queue_families: Vec<vk::QueueFamilyProperties>,
    defer_surface_initialization: bool,
    properties2_ext_enabled: bool,
    suitable: Suitable,
}

impl Hash for PhysicalDevice {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.physical_device.hash(state);
    }
}

impl PartialEq<Self> for PhysicalDevice {
    fn eq(&self, other: &Self) -> bool {
        self.physical_device.eq(&other.physical_device)
    }
}

impl Eq for PhysicalDevice {}

impl PhysicalDevice {
    pub fn enable_extension_if_present(&mut self, extension: impl Into<String>) -> bool {
        let extension = extension.into();

        if self.available_extensions.contains(&extension) {
            self.extensions_to_enable.insert(extension)
        } else {
            false
        }
    }

    pub fn enable_extensions_if_present<T: Eq + Hash + Into<String>, I: IntoIterator<Item = T>>(
        &mut self,
        extensions: I,
    ) -> bool {
        let extensions = extensions.into_iter().map(Into::into);
        let extensions = HashSet::from_iter(extensions);
        let intersection: HashSet<_> = self
            .available_extensions
            .intersection(&extensions)
            .cloned()
            .collect();

        if intersection.len() == extensions.len() {
            self.extensions_to_enable.extend(intersection);
            true
        } else {
            false
        }
    }
}

struct PhysicalDeviceInstanceInfo<'a> {
    instance: &'a ash::Instance,
    surface: Option<vk::SurfaceKHR>,
    surface_instance: Option<&'a khr::surface::Instance>,
    version: u32,
    headless: bool,
    properties2_ext_enabled: bool,
}

#[derive(Debug)]
struct SelectionCriteria<'a> {
    name: String,
    preferred_device_type: PreferredDeviceType,
    allow_any_type: bool,
    require_present: bool,
    require_dedicated_transfer_queue: bool,
    require_dedicated_compute_queue: bool,
    require_separate_transfer_queue: bool,
    require_separate_compute_queue: bool,
    required_mem_size: vk::DeviceSize,
    required_extensions: Vec<String>,
    required_version: u32,
    required_features: vk::PhysicalDeviceFeatures,
    required_features2: vk::PhysicalDeviceFeatures2<'a>,

    // extended_features_chain
    defer_surface_initialization: bool,
    use_first_gpu_unconditionally: bool,
    enable_portability_subset: bool,
}

impl Default for SelectionCriteria<'_> {
    fn default() -> Self {
        Self {
            name: String::new(),
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
    ) -> PhysicalDeviceSelector<'a> {
        let enable_portability_subset = cfg!(feature = "portability");
        Self {
            instance_info: PhysicalDeviceInstanceInfo {
                instance: instance.as_ref(),
                surface_instance: instance.surface_instance.as_ref(),
                surface: instance.surface,
                version: instance.instance_version,
                headless: instance.surface_instance.is_none(),
                properties2_ext_enabled: instance.properties2_ext_enabled,
            },
            selection_criteria: SelectionCriteria {
                require_present: instance.surface_instance.is_some(),
                required_version: instance.api_version,
                enable_portability_subset,
                ..Default::default()
            },
        }
    }

    pub fn surface(mut self, surface: vk::SurfaceKHR) -> Self {
        self.instance_info.surface.replace(surface);
        self
    }

    pub fn name(mut self, name: impl Into<String>) -> Self {
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

    pub fn select_first_device_unconditionally(mut self, select: bool) -> Self {
        self.selection_criteria.use_first_gpu_unconditionally = select;
        self
    }

    fn set_is_suitable(&self, device: &mut PhysicalDevice) {
        let criteria = &self.selection_criteria;

        if !criteria.name.is_empty() && Cow::Borrowed(&criteria.name)
            != device
                .properties
                .device_name_as_c_str()
                .expect("device name should be correct cstr")
                .to_string_lossy()
        {
            device.suitable = Suitable::No;
            return;
        };

        if criteria.required_version > device.properties.api_version {
            device.suitable = Suitable::No;
            return;
        }

        let dedicated_compute = get_dedicated_queue_index(
            &device.queue_families,
            vk::QueueFlags::COMPUTE,
            vk::QueueFlags::TRANSFER,
        );

        let dedicated_transfer = get_dedicated_queue_index(
            &device.queue_families,
            vk::QueueFlags::TRANSFER,
            vk::QueueFlags::COMPUTE,
        );

        let separate_compute = get_separate_queue_index(
            &device.queue_families,
            vk::QueueFlags::COMPUTE,
            vk::QueueFlags::TRANSFER,
        );

        let separate_transfer = get_separate_queue_index(
            &device.queue_families,
            vk::QueueFlags::TRANSFER,
            vk::QueueFlags::COMPUTE,
        );

        let present_queue = get_present_queue_index(
            self.instance_info.surface_instance,
            &device.physical_device,
            self.instance_info.surface,
            &device.queue_families
        );

        if criteria.require_dedicated_compute_queue && dedicated_compute.is_none() {
            device.suitable = Suitable::No;
            return;
        }

        if criteria.require_dedicated_transfer_queue && dedicated_transfer.is_none() {
            device.suitable = Suitable::No;
            return;
        }

        if criteria.require_separate_transfer_queue && separate_transfer.is_none() {
            device.suitable = Suitable::No;
            return;
        }

        if criteria.require_separate_compute_queue && separate_compute.is_none() {
            device.suitable = Suitable::No;
            return;
        }

        if criteria.require_present && present_queue.is_none() && !criteria.defer_surface_initialization {
            device.suitable = Suitable::No;
            return;
        }

        todo!()
    }

    fn populate_device_details(
        &self,
        vk_phys_device: vk::PhysicalDevice,
    ) -> crate::Result<PhysicalDevice> {
        let instance_info = &self.instance_info;
        let criteria = &self.selection_criteria;

        let mut physical_device = PhysicalDevice {
            physical_device: vk_phys_device,
            surface: instance_info.surface,
            defer_surface_initialization: criteria.defer_surface_initialization,
            instance_version: instance_info.version,
            queue_families: unsafe {
                instance_info
                    .instance
                    .get_physical_device_queue_family_properties(vk_phys_device)
            },
            properties: unsafe {
                instance_info
                    .instance
                    .get_physical_device_properties(vk_phys_device)
            },
            features: unsafe {
                instance_info
                    .instance
                    .get_physical_device_features(vk_phys_device)
            },
            memory_properties: unsafe {
                instance_info
                    .instance
                    .get_physical_device_memory_properties(vk_phys_device)
            },
            properties2_ext_enabled: instance_info.properties2_ext_enabled,
            ..Default::default()
        };

        physical_device.name = physical_device
            .properties
            .clone()
            .device_name_as_c_str()
            .map_err(anyhow::Error::msg)?
            .to_string_lossy()
            .to_string();

        let available_extensions = unsafe {
            instance_info
                .instance
                .enumerate_device_extension_properties(vk_phys_device)
        };

        let Ok(available_extensions) = available_extensions else {
            return Ok(physical_device);
        };

        let available_extensions_names = available_extensions
            .into_iter()
            .map(|e| {
                e.extension_name_as_c_str()
                    .expect("Extension name should be correct null-terminated string")
                    .to_string_lossy()
                    .to_string()
            })
            .collect::<Vec<_>>();

        physical_device
            .available_extensions
            .extend(available_extensions_names);

        physical_device.properties2_ext_enabled = instance_info.properties2_ext_enabled;

        Ok(physical_device)
    }

    fn select_devices(&self) -> crate::Result<HashSet<PhysicalDevice>> {
        let criteria = &self.selection_criteria;
        let instance_info = &self.instance_info;
        if criteria.require_present
            && !criteria.defer_surface_initialization
            && instance_info.surface == None
        {
            return Err(crate::PhysicalDeviceError::NoSurfaceProvided.into());
        };

        let physical_devices = unsafe { instance_info.instance.enumerate_physical_devices() }
            .map_err(|_| crate::PhysicalDeviceError::FailedToEnumeratePhysicalDevices)?;
        if physical_devices.is_empty() {
            return Err(crate::PhysicalDeviceError::NoPhysicalDevicesFound.into());
        };

        let fill_out_phys_dev_with_criteria = |physical_device: &mut PhysicalDevice| {
            physical_device.features = criteria.required_features;
            let mut portability_ext_available = false;
            let portability_name = vk::KHR_PORTABILITY_SUBSET_NAME
                .to_string_lossy()
                .to_string();
            for ext in &physical_device.available_extensions {
                if criteria.enable_portability_subset && ext == &portability_name {
                    portability_ext_available = true;
                }
            }

            physical_device.extensions_to_enable.clear();
            physical_device
                .extensions_to_enable
                .extend(criteria.required_extensions.clone());

            if portability_ext_available {
                physical_device
                    .extensions_to_enable
                    .insert(portability_name);
            }
        };

        if criteria.use_first_gpu_unconditionally {
            let mut device = self.populate_device_details(physical_devices[0])?;
            fill_out_phys_dev_with_criteria(&mut device);
            return Ok(HashSet::from([device]));
        };

        let physical_devices = physical_devices.into_iter().filter_map(|p| {
            let mut phys_dev = self.populate_device_details(p).ok();

            if let Some(phys_dev) = phys_dev.as_mut() {
                self.set_is_suitable(phys_dev);
            }
            phys_dev
        }).collect::<HashSet<_>>();

        todo!()
    }

    pub fn select(self) -> crate::Result<PhysicalDevice> {
        Ok(self.select_devices().unwrap().into_iter().next().unwrap())
    }
}
