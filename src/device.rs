use crate::Instance;
use ash::{khr, vk};
use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashSet};
use std::ffi::{c_void, CStr, CString};
use std::fmt::Debug;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::Deref;
use ash::vk::{AllocationCallbacks, BaseOutStructure};

fn supports_features(
    supported: &vk::PhysicalDeviceFeatures,
    requested: &vk::PhysicalDeviceFeatures,
    features_supported: &GenericFeatureChain<'_>,
    features_requested: &GenericFeatureChain<'_>,
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

    features_supported.match_all(features_requested)
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

fn check_device_extension_support(
    available_extensions: &BTreeSet<Cow<'_, str>>,
    required_extensions: &BTreeSet<String>
) -> BTreeSet<String> {
    let mut extensions_to_enable = BTreeSet::new();

    for avail_ext in available_extensions {
        for req_ext in required_extensions {
            if avail_ext == req_ext {
                extensions_to_enable.insert(req_ext.to_string());
                break;
            }
        }
    }

    extensions_to_enable
}

#[repr(u8)]
#[derive(Default, Debug, Eq, PartialEq, Ord, PartialOrd, Copy, Clone)]
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
pub struct PhysicalDevice<'a> {
    name: String,
    physical_device: vk::PhysicalDevice,
    surface: Option<vk::SurfaceKHR>,

    features: vk::PhysicalDeviceFeatures,
    properties: vk::PhysicalDeviceProperties,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
    instance_version: u32,
    extensions_to_enable: BTreeSet<Cow<'a, str>>,
    available_extensions: BTreeSet<Cow<'a, str>>,
    queue_families: Vec<vk::QueueFamilyProperties>,
    defer_surface_initialization: bool,
    properties2_ext_enabled: bool,
    suitable: Suitable,
    supported_features_chain: GenericFeatureChain<'a>
}

impl Eq for PhysicalDevice<'_> {}

impl PartialEq<Self> for PhysicalDevice<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name.eq(&other.name)
        && self.physical_device.eq(&other.physical_device)
        && self.suitable.eq(&other.suitable)
    }
}

impl PartialOrd for PhysicalDevice<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.suitable.partial_cmp(&other.suitable)
    }
}

impl Ord for PhysicalDevice<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.suitable.cmp(&other.suitable)
    }
}

impl<'a> PhysicalDevice<'a> {
    pub fn enable_extension_if_present(&mut self, extension: impl Into<Cow<'a, str>>) -> bool {
        let extension = extension.into();

        if self.available_extensions.contains(&extension) {
            self.extensions_to_enable.insert(extension)
        } else {
            false
        }
    }

    pub fn enable_extensions_if_present<T: Eq + Hash + Into<Cow<'a, str>>, I: IntoIterator<Item = T>>(
        &mut self,
        extensions: I,
    ) -> bool {
        let extensions = extensions.into_iter().map(Into::into);
        let extensions = BTreeSet::from_iter(extensions);
        let intersection: BTreeSet<_> = self
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

impl GenericFeaturesPNextNode<'_> {
    const FIELD_CAPACITY: usize = 256;

    fn combine(&mut self, other: &GenericFeaturesPNextNode) {
        assert_eq!(self.s_type, other.s_type);

        for i in 0..GenericFeaturesPNextNode::FIELD_CAPACITY {
            self.fields[i] = vk::Bool32::from(self.fields[i] == vk::TRUE || other.fields[i] == vk::TRUE);
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
struct GenericFeaturesPNextNode<'a> {
    s_type: vk::StructureType,
    p_next: *mut c_void,
    fields: [vk::Bool32; GenericFeaturesPNextNode::FIELD_CAPACITY],
    _marker: PhantomData<&'a ()>,
}

impl<'a, T> From<T> for GenericFeaturesPNextNode<'a>
where T: vk::ExtendsPhysicalDeviceFeatures2 + 'a {
    fn from(value: T) -> Self {
        assert!(size_of::<T>() <= size_of::<Self>());
        let mut this = GenericFeaturesPNextNode {
            s_type: vk::StructureType::from_raw(0),
            p_next: std::ptr::null_mut(),
            fields: [0; 256],
            _marker: PhantomData
        };
        let size = size_of::<T>();
        unsafe {
            std::ptr::copy_nonoverlapping(
                &value as *const T as *const u8,
                &mut this as *mut Self as *mut u8,
                size,
            );
        }
        this
    }
}

#[derive(Debug, Clone, Default)]
struct GenericFeatureChain<'a> {
    nodes: Vec<GenericFeaturesPNextNode<'a>>,
}

impl<'a> Deref for GenericFeatureChain<'a> {
    type Target = Vec<GenericFeaturesPNextNode<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.nodes
    }
}

impl<'a> GenericFeatureChain<'a> {
    fn new() -> Self {
        Self {
            nodes: vec![]
        }
    }

    fn add(&mut self, feature: impl vk::ExtendsPhysicalDeviceFeatures2 + 'a) {
        let new_node = GenericFeaturesPNextNode::from(feature);

        for node in &mut self.nodes {
            if new_node.s_type == node.s_type {
                node.combine(&new_node);
                return;
            }
        };

        self.nodes.push(new_node);
    }

    fn match_all(&self, features_requested: &GenericFeatureChain) -> bool {
        if features_requested.nodes.len() != self.nodes.len() {
            return false;
        }

        let features_requested = features_requested.nodes.as_slice();
        let features = self.nodes.as_slice();

        for (requested_node, node) in features_requested.iter().zip(features) {
            if !match_node(requested_node, node) {
                return false;
            }
        };

        true
    }

    fn chain_up(&mut self, features: &mut vk::PhysicalDeviceFeatures2) {
        let mut prev = std::ptr::null_mut::<GenericFeaturesPNextNode>();
        for extension in self.nodes.iter_mut() {
            if !prev.is_null() {
                unsafe { (*prev).p_next = extension as *mut GenericFeaturesPNextNode as _ };
            }
            prev = extension as *mut GenericFeaturesPNextNode;
        }
        features.p_next = if !self.nodes.is_empty() {
            <*mut GenericFeaturesPNextNode>::cast(self.nodes.get_mut(0).unwrap()) as _
        } else {
            std::ptr::null_mut()
        };
    }
}

fn match_node(requested: &GenericFeaturesPNextNode, supported: &GenericFeaturesPNextNode) -> bool {
    assert_eq!(requested.s_type, supported.s_type);

    for i in 0..GenericFeaturesPNextNode::FIELD_CAPACITY {
        if requested.fields[i] == vk::TRUE && supported.fields[i] == vk::FALSE {
            return false
        }
    }

    true
}

pub trait Feature2: vk::ExtendsPhysicalDeviceFeatures2 + Debug {}

impl<T> Feature2 for T where T: vk::ExtendsPhysicalDeviceFeatures2 + Debug {}

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
    required_extensions: BTreeSet<String>,
    required_version: u32,
    required_features: vk::PhysicalDeviceFeatures,
    requested_features_chain: RefCell<GenericFeatureChain<'a>>,
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
            required_extensions: BTreeSet::new(),
            required_version: vk::API_VERSION_1_0,
            required_features: vk::PhysicalDeviceFeatures::default(),
            defer_surface_initialization: false,
            use_first_gpu_unconditionally: false,
            enable_portability_subset: true,
            requested_features_chain: RefCell::new(GenericFeatureChain::new()),
        }
    }
}

unsafe fn ptr_chain_iter<T: ?Sized>(
    ptr: &mut T,
) -> impl Iterator<Item = *mut BaseOutStructure<'_>> {
    let ptr = <*mut T>::cast::<BaseOutStructure<'_>>(ptr);
    (0..).scan(ptr, |p_ptr, _| {
        if p_ptr.is_null() {
            return None;
        }
        let n_ptr = (**p_ptr).p_next;
        let old = *p_ptr;
        *p_ptr = n_ptr;
        Some(old)
    })
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

    pub fn add_required_extension_feature<T: vk::ExtendsPhysicalDeviceFeatures2 + 'a>(mut self, feature: T) -> Self {
        self.selection_criteria.requested_features_chain.borrow_mut().add(feature);
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

        let required_extensions_supported = check_device_extension_support(
            &device.available_extensions,
            &criteria.required_extensions
        );

        if required_extensions_supported.len() != criteria.required_extensions.len() {
            device.suitable = Suitable::No;
            return;
        }

        if !criteria.defer_surface_initialization && criteria.require_present {
            if let Some((surface_instance, surface)) = self.instance_info.surface_instance.zip(self.instance_info.surface) {
                let formats = unsafe { surface_instance.get_physical_device_surface_formats(device.physical_device, surface) };
                let Ok(formats) = formats else {
                    device.suitable = Suitable::No;
                    return;
                };

                let present_modes = unsafe { surface_instance.get_physical_device_surface_present_modes(device.physical_device, surface) };
                let Ok(present_modes) = present_modes else {
                    device.suitable = Suitable::No;
                    return;
                };

                if present_modes.is_empty() || formats.is_empty() {
                    device.suitable = Suitable::No;
                    return;
                }
            };
        };

        let preferred_device_type = vk::PhysicalDeviceType::from_raw(criteria.preferred_device_type as u8 as i32);
        if !criteria.allow_any_type && device.properties.device_type != preferred_device_type {
            device.suitable = Suitable::Partial;
        }

        let required_features_supported = supports_features(
            &device.features,
            &criteria.required_features,
            &device.supported_features_chain,
            &criteria.requested_features_chain.borrow()
        );
        if !required_features_supported {
            device.suitable = Suitable::No;
            return;
        }

        for memory_heap in device.memory_properties.memory_heaps {
            if memory_heap.flags.contains(vk::MemoryHeapFlags::DEVICE_LOCAL) {
                if memory_heap.size < criteria.required_mem_size {
                    device.suitable = Suitable::No;
                    return;
                }
            }
        }
    }

    fn populate_device_details(
        &'a self,
        vk_phys_device: vk::PhysicalDevice,
    ) -> crate::Result<PhysicalDevice<'a>> {
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
            .collect::<BTreeSet<_>>();

        physical_device
            .available_extensions
            .extend(available_extensions_names.iter().map(|s| Cow::Owned(s.clone())));

        physical_device.properties2_ext_enabled = instance_info.properties2_ext_enabled;

        let mut requested_features_chain_clone = criteria.requested_features_chain.clone();
        let instance_is_11 = instance_info.version >= vk::API_VERSION_1_1;
        if !requested_features_chain_clone.borrow().is_empty() && (instance_is_11 || instance_info.properties2_ext_enabled) {
            let mut local_features = vk::PhysicalDeviceFeatures2::default();

            {
                let mut supported_features_chain = requested_features_chain_clone.borrow_mut();
                supported_features_chain.chain_up(&mut local_features);
            }

            unsafe { instance_info.instance.get_physical_device_features2(physical_device.physical_device, &mut local_features) };

            physical_device.supported_features_chain = requested_features_chain_clone.into_inner();
        }

        Ok(physical_device)
    }

    fn select_devices(&'a self) -> crate::Result<BTreeSet<PhysicalDevice<'a>>> {
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
                .to_string_lossy();
            for ext in &physical_device.available_extensions {
                if criteria.enable_portability_subset && ext == &portability_name {
                    portability_ext_available = true;
                }
            }

            physical_device.extensions_to_enable.clear();
            physical_device
                .extensions_to_enable
                .extend(criteria.required_extensions.iter().map(|s| Cow::Owned(s.clone())));

            if portability_ext_available {
                physical_device
                    .extensions_to_enable
                    .insert(portability_name);
            }
        };

        if criteria.use_first_gpu_unconditionally {
            let mut device = self.populate_device_details(physical_devices[0])?;
            fill_out_phys_dev_with_criteria(&mut device);
            return Ok(BTreeSet::from([device]));
        };

        let physical_devices = physical_devices.into_iter().filter_map(|p| {
            let mut phys_dev = self.populate_device_details(p).ok();

            if let Some(phys_dev) = phys_dev.as_mut() {
                self.set_is_suitable(phys_dev);
            }

            phys_dev.and_then(|mut phys_dev| {
                if phys_dev.suitable == Suitable::No {
                    None
                } else {
                    fill_out_phys_dev_with_criteria(&mut phys_dev);

                    println!("AVAILABLE: {:#?}", phys_dev.supported_features_chain);
                    println!("REQUESTED: {:#?}", criteria.requested_features_chain);
                    Some(phys_dev)
                }
            })
        }).collect::<BTreeSet<_>>();

        Ok(physical_devices)
    }

    pub fn select(&'a self) -> crate::Result<PhysicalDevice<'a>> {
        let devices = self.select_devices()?;

        if devices.is_empty() {
            Err(crate::PhysicalDeviceError::NoSuitableDevice.into())
        } else {
            Ok(unsafe { devices.into_iter().next().unwrap_unchecked() })
        }
    }
}

fn cow_to_c_cow(cow: Cow<'_, str>) -> Cow<'_, CStr> {
    match cow {
        Cow::Borrowed(s) => {
            // Check if `s` is a valid C string
            if let Ok(c_str) = CStr::from_bytes_with_nul(s.as_bytes()) {
                Cow::Borrowed(c_str)
            } else {
                // Convert to CString, appending a null byte
                let c_string = CString::new(s).expect("Invalid C string");
                Cow::Owned(c_string)
            }
        }
        Cow::Owned(s) => {
            // Convert owned String to CString
            let c_string = CString::new(s).expect("Invalid C string");
            Cow::Owned(c_string)
        }
    }
}

pub struct DeviceBuilder<'a> {
    instance: &'a Instance<'a>,
    physical_device: &'a PhysicalDevice<'a>,
    allocation_callbacks: Option<AllocationCallbacks<'a>>,
    // TODO: pNext chains for features
    // TODO: queue descriptions
}

impl<'a> DeviceBuilder<'a> {
    pub fn new(physical_device: &'a PhysicalDevice<'a>, instance: &'a Instance<'a>) -> DeviceBuilder<'a> {
        Self {
            physical_device,
            allocation_callbacks: None,
            instance
        }
    }

    pub fn allocation_callbacks(mut self, allocation_callbacks: AllocationCallbacks<'a>) -> Self {
        self.allocation_callbacks.replace(allocation_callbacks);
        self
    }

    pub fn build(self) -> crate::Result<Device<'a>> {
        // TODO: custom queue setup
        // (index, priorities)
        let queue_descriptions = self.physical_device.queue_families.iter().enumerate().map(|(index, _)| (index, [1.])).collect::<Vec<_>>();

        let queue_create_infos = queue_descriptions
            .iter()
            .map(|(index, priorities)| {
                let queue_create_info = vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(*index as u32)
                    .queue_priorities(priorities);
                queue_create_info
            })
            .collect::<Vec<_>>();
        let extensions_to_enable = self.physical_device.extensions_to_enable.iter().map(|e| cow_to_c_cow(e.clone())).collect::<Vec<_>>();

        let mut extensions_to_enable = extensions_to_enable.iter().map(|e| e.as_ptr()).collect::<Vec<_>>();
        if self.physical_device.surface.is_some() || self.physical_device.defer_surface_initialization {
            extensions_to_enable.push(vk::KHR_SWAPCHAIN_NAME.as_ptr());
        }

        let device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&extensions_to_enable);

        let mut device = unsafe { self.instance.as_ref().create_device(self.physical_device.physical_device, &device_create_info, self.allocation_callbacks.as_ref()) }?;

        let physical_device = self.physical_device;
        
        Ok(Device {
            device,
            physical_device: physical_device,
            surface: physical_device.surface,
            allocation_callbacks: self.allocation_callbacks,
            queue_families: &physical_device.queue_families,
            instance_version: physical_device.instance_version,
        })
    }
}

pub struct Device<'a> {
    device: ash::Device,
    physical_device: &'a PhysicalDevice<'a>,
    surface: Option<vk::SurfaceKHR>,
    queue_families: &'a [vk::QueueFamilyProperties],
    allocation_callbacks: Option<AllocationCallbacks<'a>>,
    instance_version: u32,
}

impl<'a> AsRef<ash::Device> for Device<'a> {
    fn as_ref(&self) -> &ash::Device {
        &self.device
    }
}

impl<'a> Device<'a> {

}