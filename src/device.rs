use crate::Instance;
use ash::vk;
use std::borrow::Cow;
use std::collections::HashSet;
use std::hash::{Hash, Hasher};
// #[derive(Debug)]
// struct Features {
//     robust_buffer_access: bool,
//     full_draw_index_uint32: bool,
//     image_cube_array: bool,
//     independent_blend: bool,
//     geometry_shader: bool,
//     tessellation_shader: bool,
//     sample_rate_shading: bool,
//     dual_src_blend: bool,
//     logic_op: bool,
//     multi_draw_indirect: bool,
//     draw_indirect_first_instance: bool,
//     depth_clamp: bool,
//     depth_bias_clamp: bool,
//     fill_mode_non_solid: bool,
//     depth_bounds: bool,
//     wide_lines: bool,
//     large_points: bool,
//     alpha_to_one: bool,
//     multi_viewport: bool,
//     sampler_anisotropy: bool,
//     texture_compression_etc2: bool,
//     texture_compression_astc_ldr: bool,
//     texture_compression_bc: bool,
//     occlusion_query_precise: bool,
//     pipeline_statistics_query: bool,
//     vertex_pipeline_stores_and_atomics: bool,
//     fragment_stores_and_atomics: bool,
//     shader_tessellation_and_geometry_point_size: bool,
//     shader_image_gather_extended: bool,
//     shader_storage_image_extended_formats: bool,
//     shader_storage_image_multisample: bool,
//     shader_storage_image_read_without_format: bool,
//     shader_storage_image_write_without_format: bool,
//     shader_uniform_buffer_array_dynamic_indexing: bool,
//     shader_sampled_image_array_dynamic_indexing: bool,
//     shader_storage_buffer_array_dynamic_indexing: bool,
//     shader_storage_image_array_dynamic_indexing: bool,
//     shader_clip_distance: bool,
//     shader_cull_distance: bool,
//     shader_float64: bool,
//     shader_int64: bool,
//     shader_int16: bool,
//     shader_resource_residency: bool,
//     shader_resource_min_lod: bool,
//     sparse_binding: bool,
//     sparse_residency_buffer: bool,
//     sparse_residency_image2_d: bool,
//     sparse_residency_image3_d: bool,
//     sparse_residency2_samples: bool,
//     sparse_residency4_samples: bool,
//     sparse_residency8_samples: bool,
//     sparse_residency16_samples: bool,
//     sparse_residency_aliased: bool,
//     variable_multisample_rate: bool,
//     inherited_queries: bool,
// }
//
// impl From<vk::PhysicalDeviceFeatures> for Features {
//     fn from(features: vk::PhysicalDeviceFeatures) -> Self {
//         Self {
//             robust_buffer_access: features.robust_buffer_access == vk::TRUE,
//             full_draw_index_uint32: features.full_draw_index_uint32 == vk::TRUE,
//             image_cube_array: features.image_cube_array == vk::TRUE,
//             independent_blend: features.independent_blend == vk::TRUE,
//             geometry_shader: features.geometry_shader == vk::TRUE,
//             tessellation_shader: features.tessellation_shader == vk::TRUE,
//             sample_rate_shading: features.sample_rate_shading == vk::TRUE,
//             dual_src_blend: features.dual_src_blend == vk::TRUE,
//             logic_op: features.logic_op == vk::TRUE,
//             multi_draw_indirect: features.logic_op == vk::TRUE,
//             draw_indirect_first_instance: features.draw_indirect_first_instance == vk::TRUE,
//             depth_clamp: features.depth_clamp == vk::TRUE,
//             depth_bias_clamp: features.depth_bias_clamp == vk::TRUE,
//             fill_mode_non_solid: features.fill_mode_non_solid == vk::TRUE,
//             depth_bounds: features.depth_bounds == vk::TRUE,
//             wide_lines: features.wide_lines == vk::TRUE,
//             large_points: features.large_points == vk::TRUE,
//             alpha_to_one: features.alpha_to_one == vk::TRUE,
//             multi_viewport: features.multi_viewport == vk::TRUE,
//             sampler_anisotropy: features.sampler_anisotropy == vk::TRUE,
//             texture_compression_etc2: features.texture_compression_etc2 == vk::TRUE,
//             texture_compression_astc_ldr: features.texture_compression_astc_ldr == vk::TRUE,
//             texture_compression_bc: features.texture_compression_bc == vk::TRUE,
//             occlusion_query_precise: features.occlusion_query_precise == vk::TRUE,
//             pipeline_statistics_query: features.pipeline_statistics_query == vk::TRUE,
//             vertex_pipeline_stores_and_atomics: features.vertex_pipeline_stores_and_atomics == vk::TRUE,
//             fragment_stores_and_atomics: features.fragment_stores_and_atomics == vk::TRUE,
//             shader_tessellation_and_geometry_point_size: features.shader_tessellation_and_geometry_point_size == vk::TRUE,
//             shader_image_gather_extended: features.shader_image_gather_extended == vk::TRUE,
//             shader_storage_image_extended_formats: features.shader_storage_image_extended_formats == vk::TRUE,
//             shader_storage_image_multisample: features.shader_storage_image_multisample == vk::TRUE,
//             shader_storage_image_read_without_format: features.shader_storage_image_read_without_format == vk::TRUE,
//             shader_storage_image_write_without_format: features.shader_storage_image_write_without_format == vk::TRUE,
//             shader_uniform_buffer_array_dynamic_indexing: features.shader_uniform_buffer_array_dynamic_indexing == vk::TRUE,
//
//         }
//     }
// }

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
    surface: vk::SurfaceKHR,
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
        surface: Option<vk::SurfaceKHR>,
    ) -> PhysicalDeviceSelector<'a> {
        let enable_portability_subset = cfg!(feature = "portability");
        Self {
            instance_info: PhysicalDeviceInstanceInfo {
                instance: instance.as_ref(),
                surface: surface.unwrap_or_default(),
                version: instance.instance_version,
                headless: instance.headless,
                properties2_ext_enabled: instance.properties2_ext_enabled,
            },
            selection_criteria: SelectionCriteria {
                require_present: !instance.headless,
                required_version: instance.api_version,
                enable_portability_subset,
                ..Default::default()
            },
        }
    }

    pub fn surface(mut self, surface: vk::SurfaceKHR) -> Self {
        self.instance_info.surface = surface;
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

    fn is_device_suitable(&self, device: &PhysicalDevice) -> Suitable {
        let mut suitable = Suitable::Yes;
        let criteria = &self.selection_criteria;

        if Cow::Borrowed(&criteria.name) != device.properties.device_name_as_c_str().expect("device name should be correct cstr").to_string_lossy() {
            return Suitable::No;
        };

        todo!()
    }

    fn populate_device_details(
        &self,
        vk_phys_device: vk::PhysicalDevice,
    ) -> crate::Result<PhysicalDevice> {
        let instance_info = &self.instance_info;
        let criteria = &self.selection_criteria;

        let mut physical_device = PhysicalDevice::default();
        physical_device.physical_device = vk_phys_device;
        physical_device.surface = instance_info.surface;
        physical_device.defer_surface_initialization = criteria.defer_surface_initialization;
        physical_device.instance_version = instance_info.version;

        let queue_families = unsafe {
            instance_info
                .instance
                .get_physical_device_queue_family_properties(vk_phys_device)
        };
        physical_device.queue_families = queue_families;

        physical_device.properties = unsafe {
            instance_info
                .instance
                .get_physical_device_properties(vk_phys_device)
        };

        physical_device.features = unsafe {
            instance_info
                .instance
                .get_physical_device_features(vk_phys_device)
        };

        physical_device.memory_properties = unsafe {
            instance_info
                .instance
                .get_physical_device_memory_properties(vk_phys_device)
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
            && instance_info.surface == vk::SurfaceKHR::default()
        {
            return Err(crate::PhysicalDeviceError::NoSurfaceProvided.into());
        };

        let physical_devices = unsafe { instance_info.instance.enumerate_physical_devices() }
            .map_err(|_| crate::PhysicalDeviceError::FailedToEnumeratePhysicalDevices)?;
        if physical_devices.is_empty() {
            return Err(crate::PhysicalDeviceError::NoPhysicalDevicesFound.into());
        };

        let fill_out_phys_dev_with_criteria = |physical_device: &mut PhysicalDevice| {
            physical_device.features = criteria.required_features.clone();
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

        let physical_devices = physical_devices
            .into_iter()
            .filter_map(|p| {
                let mut phys_dev = self.populate_device_details(p).ok();

                phys_dev.map(|phys_dev| {

                })
            });

        todo!()
    }

    pub fn select(self) -> crate::Result<PhysicalDevice> {


        todo!()
    }
}
