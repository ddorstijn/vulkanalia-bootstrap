use crate::Instance;
use crate::version::Version;
use ash::vk::AllocationCallbacks;
use ash::{khr, vk};
use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::ffi::{CStr, CString};
use std::fmt::Debug;
use std::hash::Hash;
use std::hint::unreachable_unchecked;
use std::ops::Deref;
use std::sync::Arc;

fn supports_features(
    supported: &vk::PhysicalDeviceFeatures,
    requested: &vk::PhysicalDeviceFeatures,
    features_supported: &GenericFeatureChain,
    features_requested: &GenericFeatureChain,
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
    families.iter().position(|f| {
        f.queue_flags.contains(desired_flags)
            && !f.queue_flags.contains(vk::QueueFlags::GRAPHICS)
            && !f.queue_flags.contains(undesired_flags)
    })
}

fn get_present_queue_index(
    instance: &Option<khr::surface::Instance>,
    device: vk::PhysicalDevice,
    surface: Option<vk::SurfaceKHR>,
    families: &[vk::QueueFamilyProperties],
) -> Option<usize> {
    for (i, _) in families.iter().enumerate() {
        if let Some((surface, instance)) = surface.zip(instance.as_ref()) {
            let present_support =
                unsafe { instance.get_physical_device_surface_support(device, i as u32, surface) };

            if let Ok(present_support) = present_support {
                if present_support {
                    return Some(i);
                }
            }
        }
    }

    None
}

fn check_device_extension_support(
    available_extensions: &BTreeSet<Cow<'_, str>>,
    required_extensions: &BTreeSet<String>,
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
pub struct PhysicalDevice {
    name: String,
    pub physical_device: vk::PhysicalDevice,
    surface: Option<vk::SurfaceKHR>,

    features: vk::PhysicalDeviceFeatures,
    properties: vk::PhysicalDeviceProperties,
    memory_properties: vk::PhysicalDeviceMemoryProperties,
    extensions_to_enable: BTreeSet<Cow<'static, str>>,
    available_extensions: BTreeSet<Cow<'static, str>>,
    queue_families: Vec<vk::QueueFamilyProperties>,
    defer_surface_initialization: bool,
    properties2_ext_enabled: bool,
    suitable: Suitable,
    supported_features_chain: GenericFeatureChain<'static>,
    requested_features_chain: GenericFeatureChain<'static>,
}

impl AsRef<vk::PhysicalDevice> for PhysicalDevice {
    fn as_ref(&self) -> &vk::PhysicalDevice {
        &self.physical_device
    }
}

impl Eq for PhysicalDevice {}

impl PartialEq<Self> for PhysicalDevice {
    fn eq(&self, other: &Self) -> bool {
        self.name.eq(&other.name)
            && self.physical_device.eq(&other.physical_device)
            && self.suitable.eq(&other.suitable)
    }
}

impl PartialOrd for PhysicalDevice {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for PhysicalDevice {
    fn cmp(&self, other: &Self) -> Ordering {
        self.suitable.cmp(&other.suitable)
    }
}

impl PhysicalDevice {
    pub fn enable_extension_if_present(&mut self, extension: impl Into<Cow<'static, str>>) -> bool {
        let extension = extension.into();

        if self.available_extensions.contains(&extension) {
            self.extensions_to_enable.insert(extension)
        } else {
            false
        }
    }

    pub fn enable_extensions_if_present<
        T: Eq + Hash + Into<Cow<'static, str>>,
        I: IntoIterator<Item = T>,
    >(
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

// TODO: proper transmute via ash
//region vulkanfeatures
#[derive(Debug, Clone)]
pub enum VulkanPhysicalDeviceFeature2<'a> {
    PhysicalDeviceVulkan11(vk::PhysicalDeviceVulkan11Features<'a>),
    PhysicalDeviceVulkan12(vk::PhysicalDeviceVulkan12Features<'a>),
    PhysicalDeviceVulkan13(vk::PhysicalDeviceVulkan13Features<'a>),
}

fn match_features(
    requested: &VulkanPhysicalDeviceFeature2<'_>,
    supported: &VulkanPhysicalDeviceFeature2<'_>,
) -> bool {
    assert_eq!(requested.s_type(), supported.s_type());

    match (requested, supported) {
        (
            VulkanPhysicalDeviceFeature2::PhysicalDeviceVulkan11(r),
            VulkanPhysicalDeviceFeature2::PhysicalDeviceVulkan11(s),
        ) => {
            if r.storage_buffer16_bit_access == vk::TRUE
                && s.storage_buffer16_bit_access == vk::FALSE
            {
                return false;
            }
            if r.uniform_and_storage_buffer16_bit_access == vk::TRUE
                && s.uniform_and_storage_buffer16_bit_access == vk::FALSE
            {
                return false;
            }
            if r.storage_push_constant16 == vk::TRUE && s.storage_push_constant16 == vk::FALSE {
                return false;
            }
            if r.storage_input_output16 == vk::TRUE && s.storage_input_output16 == vk::FALSE {
                return false;
            }
            if r.multiview == vk::TRUE && s.multiview == vk::FALSE {
                return false;
            }
            if r.multiview_geometry_shader == vk::TRUE && s.multiview_geometry_shader == vk::FALSE {
                return false;
            }
            if r.multiview_tessellation_shader == vk::TRUE
                && s.multiview_tessellation_shader == vk::FALSE
            {
                return false;
            }
            if r.variable_pointers_storage_buffer == vk::TRUE
                && s.variable_pointers_storage_buffer == vk::FALSE
            {
                return false;
            }
            if r.variable_pointers == vk::TRUE && s.variable_pointers == vk::FALSE {
                return false;
            }
            if r.protected_memory == vk::TRUE && s.protected_memory == vk::FALSE {
                return false;
            }
            if r.sampler_ycbcr_conversion == vk::TRUE && s.sampler_ycbcr_conversion == vk::FALSE {
                return false;
            }
            if r.shader_draw_parameters == vk::TRUE && s.shader_draw_parameters == vk::FALSE {
                return false;
            }
            true
        }
        (
            VulkanPhysicalDeviceFeature2::PhysicalDeviceVulkan12(r),
            VulkanPhysicalDeviceFeature2::PhysicalDeviceVulkan12(s),
        ) => {
            if r.sampler_mirror_clamp_to_edge == vk::TRUE
                && s.sampler_mirror_clamp_to_edge == vk::FALSE
            {
                return false;
            }
            if r.draw_indirect_count == vk::TRUE && s.draw_indirect_count == vk::FALSE {
                return false;
            }
            if r.storage_buffer8_bit_access == vk::TRUE && s.storage_buffer8_bit_access == vk::FALSE
            {
                return false;
            }
            if r.uniform_and_storage_buffer8_bit_access == vk::TRUE
                && s.uniform_and_storage_buffer8_bit_access == vk::FALSE
            {
                return false;
            }
            if r.storage_push_constant8 == vk::TRUE && s.storage_push_constant8 == vk::FALSE {
                return false;
            }
            if r.shader_buffer_int64_atomics == vk::TRUE
                && s.shader_buffer_int64_atomics == vk::FALSE
            {
                return false;
            }
            if r.shader_shared_int64_atomics == vk::TRUE
                && s.shader_shared_int64_atomics == vk::FALSE
            {
                return false;
            }
            if r.shader_float16 == vk::TRUE && s.shader_float16 == vk::FALSE {
                return false;
            }
            if r.shader_int8 == vk::TRUE && s.shader_int8 == vk::FALSE {
                return false;
            }
            if r.descriptor_indexing == vk::TRUE && s.descriptor_indexing == vk::FALSE {
                return false;
            }
            if r.shader_input_attachment_array_dynamic_indexing == vk::TRUE
                && s.shader_input_attachment_array_dynamic_indexing == vk::FALSE
            {
                return false;
            }
            if r.shader_uniform_texel_buffer_array_dynamic_indexing == vk::TRUE
                && s.shader_uniform_texel_buffer_array_dynamic_indexing == vk::FALSE
            {
                return false;
            }
            if r.shader_storage_texel_buffer_array_dynamic_indexing == vk::TRUE
                && s.shader_storage_texel_buffer_array_dynamic_indexing == vk::FALSE
            {
                return false;
            }
            if r.shader_uniform_buffer_array_non_uniform_indexing == vk::TRUE
                && s.shader_uniform_buffer_array_non_uniform_indexing == vk::FALSE
            {
                return false;
            }
            if r.shader_sampled_image_array_non_uniform_indexing == vk::TRUE
                && s.shader_sampled_image_array_non_uniform_indexing == vk::FALSE
            {
                return false;
            }
            if r.shader_storage_buffer_array_non_uniform_indexing == vk::TRUE
                && s.shader_storage_buffer_array_non_uniform_indexing == vk::FALSE
            {
                return false;
            }
            if r.shader_storage_image_array_non_uniform_indexing == vk::TRUE
                && s.shader_storage_image_array_non_uniform_indexing == vk::FALSE
            {
                return false;
            }
            if r.shader_input_attachment_array_non_uniform_indexing == vk::TRUE
                && s.shader_input_attachment_array_non_uniform_indexing == vk::FALSE
            {
                return false;
            }
            if r.shader_uniform_texel_buffer_array_non_uniform_indexing == vk::TRUE
                && s.shader_uniform_texel_buffer_array_non_uniform_indexing == vk::FALSE
            {
                return false;
            }
            if r.shader_storage_texel_buffer_array_non_uniform_indexing == vk::TRUE
                && s.shader_storage_texel_buffer_array_non_uniform_indexing == vk::FALSE
            {
                return false;
            }
            if r.descriptor_binding_uniform_buffer_update_after_bind == vk::TRUE
                && s.descriptor_binding_uniform_buffer_update_after_bind == vk::FALSE
            {
                return false;
            }
            if r.descriptor_binding_sampled_image_update_after_bind == vk::TRUE
                && s.descriptor_binding_sampled_image_update_after_bind == vk::FALSE
            {
                return false;
            }
            if r.descriptor_binding_storage_image_update_after_bind == vk::TRUE
                && s.descriptor_binding_storage_image_update_after_bind == vk::FALSE
            {
                return false;
            }
            if r.descriptor_binding_storage_buffer_update_after_bind == vk::TRUE
                && s.descriptor_binding_storage_buffer_update_after_bind == vk::FALSE
            {
                return false;
            }
            if r.descriptor_binding_uniform_texel_buffer_update_after_bind == vk::TRUE
                && s.descriptor_binding_uniform_texel_buffer_update_after_bind == vk::FALSE
            {
                return false;
            }
            if r.descriptor_binding_storage_texel_buffer_update_after_bind == vk::TRUE
                && s.descriptor_binding_storage_texel_buffer_update_after_bind == vk::FALSE
            {
                return false;
            }
            if r.descriptor_binding_update_unused_while_pending == vk::TRUE
                && s.descriptor_binding_update_unused_while_pending == vk::FALSE
            {
                return false;
            }
            if r.descriptor_binding_partially_bound == vk::TRUE
                && s.descriptor_binding_partially_bound == vk::FALSE
            {
                return false;
            }
            if r.descriptor_binding_variable_descriptor_count == vk::TRUE
                && s.descriptor_binding_variable_descriptor_count == vk::FALSE
            {
                return false;
            }
            if r.runtime_descriptor_array == vk::TRUE && s.runtime_descriptor_array == vk::FALSE {
                return false;
            }
            if r.sampler_filter_minmax == vk::TRUE && s.sampler_filter_minmax == vk::FALSE {
                return false;
            }
            if r.scalar_block_layout == vk::TRUE && s.scalar_block_layout == vk::FALSE {
                return false;
            }
            if r.imageless_framebuffer == vk::TRUE && s.imageless_framebuffer == vk::FALSE {
                return false;
            }
            if r.uniform_buffer_standard_layout == vk::TRUE
                && s.uniform_buffer_standard_layout == vk::FALSE
            {
                return false;
            }
            if r.shader_subgroup_extended_types == vk::TRUE
                && s.shader_subgroup_extended_types == vk::FALSE
            {
                return false;
            }
            if r.separate_depth_stencil_layouts == vk::TRUE
                && s.separate_depth_stencil_layouts == vk::FALSE
            {
                return false;
            }
            if r.host_query_reset == vk::TRUE && s.host_query_reset == vk::FALSE {
                return false;
            }
            if r.timeline_semaphore == vk::TRUE && s.timeline_semaphore == vk::FALSE {
                return false;
            }
            if r.buffer_device_address == vk::TRUE && s.buffer_device_address == vk::FALSE {
                return false;
            }
            if r.buffer_device_address_capture_replay == vk::TRUE
                && s.buffer_device_address_capture_replay == vk::FALSE
            {
                return false;
            }
            if r.buffer_device_address_multi_device == vk::TRUE
                && s.buffer_device_address_multi_device == vk::FALSE
            {
                return false;
            }
            if r.vulkan_memory_model == vk::TRUE && s.vulkan_memory_model == vk::FALSE {
                return false;
            }
            if r.vulkan_memory_model_device_scope == vk::TRUE
                && s.vulkan_memory_model_device_scope == vk::FALSE
            {
                return false;
            }
            if r.vulkan_memory_model_availability_visibility_chains == vk::TRUE
                && s.vulkan_memory_model_availability_visibility_chains == vk::FALSE
            {
                return false;
            }
            if r.shader_output_viewport_index == vk::TRUE
                && s.shader_output_viewport_index == vk::FALSE
            {
                return false;
            }
            if r.shader_output_layer == vk::TRUE && s.shader_output_layer == vk::FALSE {
                return false;
            }
            if r.subgroup_broadcast_dynamic_id == vk::TRUE
                && s.subgroup_broadcast_dynamic_id == vk::FALSE
            {
                return false;
            }
            true
        }
        (
            VulkanPhysicalDeviceFeature2::PhysicalDeviceVulkan13(r),
            VulkanPhysicalDeviceFeature2::PhysicalDeviceVulkan13(s),
        ) => {
            if r.robust_image_access == vk::TRUE && s.robust_image_access == vk::FALSE {
                return false;
            }
            if r.inline_uniform_block == vk::TRUE && s.inline_uniform_block == vk::FALSE {
                return false;
            }
            if r.descriptor_binding_inline_uniform_block_update_after_bind == vk::TRUE
                && s.descriptor_binding_inline_uniform_block_update_after_bind == vk::FALSE
            {
                return false;
            }
            if r.pipeline_creation_cache_control == vk::TRUE
                && s.pipeline_creation_cache_control == vk::FALSE
            {
                return false;
            }
            if r.private_data == vk::TRUE && s.private_data == vk::FALSE {
                return false;
            }
            if r.shader_demote_to_helper_invocation == vk::TRUE
                && s.shader_demote_to_helper_invocation == vk::FALSE
            {
                return false;
            }
            if r.shader_terminate_invocation == vk::TRUE
                && s.shader_terminate_invocation == vk::FALSE
            {
                return false;
            }
            if r.subgroup_size_control == vk::TRUE && s.subgroup_size_control == vk::FALSE {
                return false;
            }
            if r.compute_full_subgroups == vk::TRUE && s.compute_full_subgroups == vk::FALSE {
                return false;
            }
            if r.synchronization2 == vk::TRUE && s.synchronization2 == vk::FALSE {
                return false;
            }
            if r.texture_compression_astc_hdr == vk::TRUE
                && s.texture_compression_astc_hdr == vk::FALSE
            {
                return false;
            }
            if r.shader_zero_initialize_workgroup_memory == vk::TRUE
                && s.shader_zero_initialize_workgroup_memory == vk::FALSE
            {
                return false;
            }
            if r.dynamic_rendering == vk::TRUE && s.dynamic_rendering == vk::FALSE {
                return false;
            }
            if r.shader_integer_dot_product == vk::TRUE && s.shader_integer_dot_product == vk::FALSE
            {
                return false;
            }
            if r.maintenance4 == vk::TRUE && s.maintenance4 == vk::FALSE {
                return false;
            }
            true
        }
        _ => unsafe { unreachable_unchecked() },
    }
}
impl<'a> VulkanPhysicalDeviceFeature2<'a> {
    fn as_mut(&mut self) -> &mut dyn vk::ExtendsPhysicalDeviceFeatures2 {
        match self {
            Self::PhysicalDeviceVulkan11(f) => f,
            Self::PhysicalDeviceVulkan12(f) => f,
            Self::PhysicalDeviceVulkan13(f) => f,
        }
    }

    fn combine(&mut self, other: &VulkanPhysicalDeviceFeature2<'a>) {
        assert_eq!(self.s_type(), other.s_type());

        match (self, other) {
            (
                Self::PhysicalDeviceVulkan11(f),
                VulkanPhysicalDeviceFeature2::PhysicalDeviceVulkan11(other),
            ) => {
                f.storage_buffer16_bit_access |= other.storage_buffer16_bit_access;
                f.uniform_and_storage_buffer16_bit_access |=
                    other.uniform_and_storage_buffer16_bit_access;
                f.storage_push_constant16 |= other.storage_push_constant16;
                f.storage_input_output16 |= other.storage_input_output16;
                f.multiview |= other.multiview;
                f.multiview_geometry_shader |= other.multiview_geometry_shader;
                f.multiview_tessellation_shader |= other.multiview_tessellation_shader;
                f.variable_pointers_storage_buffer |= other.variable_pointers_storage_buffer;
                f.variable_pointers |= other.variable_pointers;
                f.protected_memory |= other.protected_memory;
                f.sampler_ycbcr_conversion |= other.sampler_ycbcr_conversion;
                f.shader_draw_parameters |= other.shader_draw_parameters;
            }
            (
                Self::PhysicalDeviceVulkan12(f),
                VulkanPhysicalDeviceFeature2::PhysicalDeviceVulkan12(other),
            ) => {
                f.sampler_mirror_clamp_to_edge |= other.sampler_mirror_clamp_to_edge;
                f.draw_indirect_count |= other.draw_indirect_count;
                f.storage_buffer8_bit_access |= other.storage_buffer8_bit_access;
                f.uniform_and_storage_buffer8_bit_access |=
                    other.uniform_and_storage_buffer8_bit_access;
                f.storage_push_constant8 |= other.storage_push_constant8;
                f.shader_buffer_int64_atomics |= other.shader_buffer_int64_atomics;
                f.shader_shared_int64_atomics |= other.shader_shared_int64_atomics;
                f.shader_float16 |= other.shader_float16;
                f.shader_int8 |= other.shader_int8;
                f.descriptor_indexing |= other.descriptor_indexing;
                f.shader_input_attachment_array_dynamic_indexing |=
                    other.shader_input_attachment_array_dynamic_indexing;
                f.shader_uniform_texel_buffer_array_dynamic_indexing |=
                    other.shader_uniform_texel_buffer_array_dynamic_indexing;
                f.shader_storage_texel_buffer_array_dynamic_indexing |=
                    other.shader_storage_texel_buffer_array_dynamic_indexing;
                f.shader_uniform_buffer_array_non_uniform_indexing |=
                    other.shader_uniform_buffer_array_non_uniform_indexing;
                f.shader_sampled_image_array_non_uniform_indexing |=
                    other.shader_sampled_image_array_non_uniform_indexing;
                f.shader_storage_buffer_array_non_uniform_indexing |=
                    other.shader_storage_buffer_array_non_uniform_indexing;
                f.shader_storage_image_array_non_uniform_indexing |=
                    other.shader_storage_image_array_non_uniform_indexing;
                f.shader_input_attachment_array_non_uniform_indexing |=
                    other.shader_input_attachment_array_non_uniform_indexing;
                f.shader_uniform_texel_buffer_array_non_uniform_indexing |=
                    other.shader_uniform_texel_buffer_array_non_uniform_indexing;
                f.shader_storage_texel_buffer_array_non_uniform_indexing |=
                    other.shader_storage_texel_buffer_array_non_uniform_indexing;
                f.descriptor_binding_uniform_buffer_update_after_bind |=
                    other.descriptor_binding_uniform_buffer_update_after_bind;
                f.descriptor_binding_sampled_image_update_after_bind |=
                    other.descriptor_binding_sampled_image_update_after_bind;
                f.descriptor_binding_storage_image_update_after_bind |=
                    other.descriptor_binding_storage_image_update_after_bind;
                f.descriptor_binding_storage_buffer_update_after_bind |=
                    other.descriptor_binding_storage_buffer_update_after_bind;
                f.descriptor_binding_uniform_texel_buffer_update_after_bind |=
                    other.descriptor_binding_uniform_texel_buffer_update_after_bind;
                f.descriptor_binding_storage_texel_buffer_update_after_bind |=
                    other.descriptor_binding_storage_texel_buffer_update_after_bind;
                f.descriptor_binding_update_unused_while_pending |=
                    other.descriptor_binding_update_unused_while_pending;
                f.descriptor_binding_partially_bound |= other.descriptor_binding_partially_bound;
                f.descriptor_binding_variable_descriptor_count |=
                    other.descriptor_binding_variable_descriptor_count;
                f.runtime_descriptor_array |= other.runtime_descriptor_array;
                f.sampler_filter_minmax |= other.sampler_filter_minmax;
                f.scalar_block_layout |= other.scalar_block_layout;
                f.imageless_framebuffer |= other.imageless_framebuffer;
                f.uniform_buffer_standard_layout |= other.uniform_buffer_standard_layout;
                f.shader_subgroup_extended_types |= other.shader_subgroup_extended_types;
                f.separate_depth_stencil_layouts |= other.separate_depth_stencil_layouts;
                f.host_query_reset |= other.host_query_reset;
                f.timeline_semaphore |= other.timeline_semaphore;
                f.buffer_device_address |= other.buffer_device_address;
                f.buffer_device_address_capture_replay |=
                    other.buffer_device_address_capture_replay;
                f.buffer_device_address_multi_device |= other.buffer_device_address_multi_device;
                f.vulkan_memory_model |= other.vulkan_memory_model;
                f.vulkan_memory_model_device_scope |= other.vulkan_memory_model_device_scope;
                f.vulkan_memory_model_availability_visibility_chains |=
                    other.vulkan_memory_model_availability_visibility_chains;
                f.shader_output_viewport_index |= other.shader_output_viewport_index;
                f.shader_output_layer |= other.shader_output_layer;
                f.subgroup_broadcast_dynamic_id |= other.subgroup_broadcast_dynamic_id;
            }
            (
                Self::PhysicalDeviceVulkan13(f),
                VulkanPhysicalDeviceFeature2::PhysicalDeviceVulkan13(other),
            ) => {
                f.robust_image_access |= other.robust_image_access;
                f.inline_uniform_block |= other.inline_uniform_block;
                f.descriptor_binding_inline_uniform_block_update_after_bind |=
                    other.descriptor_binding_inline_uniform_block_update_after_bind;
                f.pipeline_creation_cache_control |= other.pipeline_creation_cache_control;
                f.private_data |= other.private_data;
                f.shader_demote_to_helper_invocation |= other.shader_demote_to_helper_invocation;
                f.shader_terminate_invocation |= other.shader_terminate_invocation;
                f.subgroup_size_control |= other.subgroup_size_control;
                f.compute_full_subgroups |= other.compute_full_subgroups;
                f.synchronization2 |= other.synchronization2;
                f.texture_compression_astc_hdr |= other.texture_compression_astc_hdr;
                f.shader_zero_initialize_workgroup_memory |=
                    other.shader_zero_initialize_workgroup_memory;
                f.dynamic_rendering |= other.dynamic_rendering;
                f.shader_integer_dot_product |= other.shader_integer_dot_product;
                f.maintenance4 |= other.maintenance4;
            }
            _ => unsafe { unreachable_unchecked() },
        }
    }

    fn s_type(&self) -> vk::StructureType {
        match self {
            Self::PhysicalDeviceVulkan11(f) => f.s_type,
            Self::PhysicalDeviceVulkan12(f) => f.s_type,
            Self::PhysicalDeviceVulkan13(f) => f.s_type,
        }
    }
}

impl<'a> From<vk::PhysicalDeviceVulkan11Features<'a>> for VulkanPhysicalDeviceFeature2<'a> {
    fn from(value: vk::PhysicalDeviceVulkan11Features<'a>) -> Self {
        Self::PhysicalDeviceVulkan11(value)
    }
}

impl<'a> From<vk::PhysicalDeviceVulkan12Features<'a>> for VulkanPhysicalDeviceFeature2<'a> {
    fn from(value: vk::PhysicalDeviceVulkan12Features<'a>) -> Self {
        Self::PhysicalDeviceVulkan12(value)
    }
}

impl<'a> From<vk::PhysicalDeviceVulkan13Features<'a>> for VulkanPhysicalDeviceFeature2<'a> {
    fn from(value: vk::PhysicalDeviceVulkan13Features<'a>) -> Self {
        Self::PhysicalDeviceVulkan13(value)
    }
}
//endregion vulkanfeatures

#[derive(Debug, Clone, Default)]
struct GenericFeatureChain<'a> {
    nodes: Vec<VulkanPhysicalDeviceFeature2<'a>>,
}

impl<'a> Deref for GenericFeatureChain<'a> {
    type Target = Vec<VulkanPhysicalDeviceFeature2<'a>>;

    fn deref(&self) -> &Self::Target {
        &self.nodes
    }
}

impl<'a> GenericFeatureChain<'a> {
    fn new() -> Self {
        Self { nodes: vec![] }
    }

    fn add(&mut self, feature: impl Into<VulkanPhysicalDeviceFeature2<'a>> + 'a) {
        let new_node = feature.into();

        for node in &mut self.nodes {
            if new_node.s_type() == node.s_type() {
                node.combine(&new_node);
                return;
            }
        }

        self.nodes.push(new_node);
    }

    fn match_all(&self, features_requested: &GenericFeatureChain) -> bool {
        if features_requested.nodes.len() != self.nodes.len() {
            return false;
        }

        let features_requested = features_requested.nodes.as_slice();
        let features = self.nodes.as_slice();

        for (requested_node, node) in features_requested.iter().zip(features) {
            if !match_features(requested_node, node) {
                return false;
            }
        }

        true
    }
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

pub struct PhysicalDeviceSelector {
    instance: Arc<Instance>,
    surface: Option<vk::SurfaceKHR>,
    selection_criteria: SelectionCriteria<'static>,
}

impl PhysicalDeviceSelector {
    pub fn new(instance: Arc<Instance>) -> PhysicalDeviceSelector {
        let enable_portability_subset = cfg!(feature = "portability");
        let require_present = instance.surface_instance.is_some();
        let required_version = instance.api_version;
        Self {
            surface: instance.surface,
            instance,
            selection_criteria: SelectionCriteria {
                require_present,
                required_version,
                enable_portability_subset,
                ..Default::default()
            },
        }
    }

    pub fn surface(mut self, surface: vk::SurfaceKHR) -> Self {
        self.surface.replace(surface);
        self
    }

    pub fn add_required_extension_feature<
        T: Into<VulkanPhysicalDeviceFeature2<'static>> + 'static,
    >(
        self,
        feature: T,
    ) -> Self {
        self.selection_criteria
            .requested_features_chain
            .borrow_mut()
            .add(feature);
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

        let device_name = device
            .properties
            .device_name_as_c_str()
            .expect("device name should be correct cstr")
            .to_string_lossy();

        if !criteria.name.is_empty() && Cow::Borrowed(&criteria.name) != device_name {
            #[cfg(feature = "tracing")]
            {
                tracing::warn!(
                    "Device {} is not suitable. Name requested: {}",
                    device_name,
                    criteria.name
                );
            }
            device.suitable = Suitable::No;
            return;
        };

        if criteria.required_version > device.properties.api_version {
            #[cfg(feature = "tracing")]
            {
                let requested_version = Version::new(criteria.required_version);
                let available_version = Version::new(device.properties.api_version);
                tracing::warn!(
                    "Device {} is not suitable. Requested version: {}, Available version: {}",
                    device_name,
                    requested_version,
                    available_version
                );
            }
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
            &self.instance.surface_instance,
            device.physical_device,
            self.surface,
            &device.queue_families,
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

        if criteria.require_present
            && present_queue.is_none()
            && !criteria.defer_surface_initialization
        {
            device.suitable = Suitable::No;
            return;
        }

        let required_extensions_supported = check_device_extension_support(
            &device.available_extensions,
            &criteria.required_extensions,
        );

        if required_extensions_supported.len() != criteria.required_extensions.len() {
            device.suitable = Suitable::No;
            return;
        }

        if !criteria.defer_surface_initialization && criteria.require_present {
            let instance = self.instance.as_ref();
            if let Some((surface_instance, surface)) =
                instance.surface_instance.as_ref().zip(self.surface)
            {
                let formats = unsafe {
                    surface_instance
                        .get_physical_device_surface_formats(device.physical_device, surface)
                };
                let Ok(formats) = formats else {
                    device.suitable = Suitable::No;
                    return;
                };

                let present_modes = unsafe {
                    surface_instance
                        .get_physical_device_surface_present_modes(device.physical_device, surface)
                };
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

        let preferred_device_type =
            vk::PhysicalDeviceType::from_raw(criteria.preferred_device_type as u8 as i32);
        if !criteria.allow_any_type && device.properties.device_type != preferred_device_type {
            device.suitable = Suitable::Partial;
        }

        let required_features_supported = supports_features(
            &device.features,
            &criteria.required_features,
            &device.supported_features_chain,
            &criteria.requested_features_chain.borrow(),
        );

        if !required_features_supported {
            device.suitable = Suitable::No;
            return;
        }

        for memory_heap in device.memory_properties.memory_heaps {
            if memory_heap
                .flags
                .contains(vk::MemoryHeapFlags::DEVICE_LOCAL)
                && memory_heap.size < criteria.required_mem_size
            {
                device.suitable = Suitable::No;
                return;
            }
        }
    }

    fn populate_device_details(
        &self,
        vk_phys_device: vk::PhysicalDevice,
    ) -> crate::Result<PhysicalDevice> {
        let instance = self.instance.as_ref();
        let criteria = &self.selection_criteria;

        let mut physical_device = PhysicalDevice {
            physical_device: vk_phys_device,
            surface: instance.surface,
            defer_surface_initialization: criteria.defer_surface_initialization,
            queue_families: unsafe {
                instance
                    .instance
                    .get_physical_device_queue_family_properties(vk_phys_device)
            },
            properties: unsafe {
                instance
                    .instance
                    .get_physical_device_properties(vk_phys_device)
            },
            features: unsafe {
                instance
                    .instance
                    .get_physical_device_features(vk_phys_device)
            },
            memory_properties: unsafe {
                instance
                    .instance
                    .get_physical_device_memory_properties(vk_phys_device)
            },
            properties2_ext_enabled: instance.properties2_ext_enabled,
            requested_features_chain: criteria.requested_features_chain.clone().into_inner(),
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
            instance
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

        physical_device.available_extensions.extend(
            available_extensions_names
                .iter()
                .map(|s| Cow::Owned(s.clone())),
        );

        physical_device.properties2_ext_enabled = instance.properties2_ext_enabled;

        let requested_features_chain = criteria.requested_features_chain.borrow();
        let instance_is_11 = instance.instance_version >= vk::API_VERSION_1_1;
        if !requested_features_chain.is_empty()
            && (instance_is_11 || instance.properties2_ext_enabled)
        {
            let mut supported_features = requested_features_chain.clone();
            let mut local_features = vk::PhysicalDeviceFeatures2::default();

            for node in supported_features.nodes.iter_mut() {
                local_features = local_features.push_next(node.as_mut());
            }

            unsafe {
                instance.instance.get_physical_device_features2(
                    physical_device.physical_device,
                    &mut local_features,
                )
            };

            physical_device.supported_features_chain = supported_features.clone();
        }

        Ok(physical_device)
    }

    fn select_devices(&self) -> crate::Result<BTreeSet<PhysicalDevice>> {
        let criteria = &self.selection_criteria;
        let instance = self.instance.as_ref();
        if criteria.require_present
            && !criteria.defer_surface_initialization
            && instance.surface.is_none()
        {
            return Err(crate::PhysicalDeviceError::NoSurfaceProvided.into());
        };

        let physical_devices = unsafe { instance.instance.enumerate_physical_devices() }
            .map_err(|_| crate::PhysicalDeviceError::FailedToEnumeratePhysicalDevices)?;
        if physical_devices.is_empty() {
            return Err(crate::PhysicalDeviceError::NoPhysicalDevicesFound.into());
        };

        let fill_out_phys_dev_with_criteria = |physical_device: &mut PhysicalDevice| {
            physical_device.features = criteria.required_features;
            let mut portability_ext_available = false;
            let portability_name = vk::KHR_PORTABILITY_SUBSET_NAME.to_string_lossy();
            for ext in &physical_device.available_extensions {
                if criteria.enable_portability_subset && ext == &portability_name {
                    portability_ext_available = true;
                }
            }

            physical_device.extensions_to_enable.clear();
            physical_device.extensions_to_enable.extend(
                criteria
                    .required_extensions
                    .iter()
                    .map(|s| Cow::Owned(s.clone())),
            );

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

        let physical_devices = physical_devices
            .into_iter()
            .filter_map(|p| {
                let mut phys_dev = self.populate_device_details(p).ok();

                if let Some(phys_dev) = phys_dev.as_mut() {
                    self.set_is_suitable(phys_dev);
                }

                phys_dev.and_then(|mut phys_dev| {
                    if phys_dev.suitable == Suitable::No {
                        None
                    } else {
                        fill_out_phys_dev_with_criteria(&mut phys_dev);

                        Some(phys_dev)
                    }
                })
            })
            .collect::<BTreeSet<_>>();

        Ok(physical_devices)
    }

    pub fn select(self) -> crate::Result<PhysicalDevice> {
        let devices = self.select_devices()?;
        #[cfg(feature = "tracing")]
        {
            tracing::debug!(
                "Device suitability: {:#?}",
                devices
                    .iter()
                    .map(|d| (&d.name, &d.suitable))
                    .collect::<Vec<_>>()
            );
        }

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

pub struct DeviceBuilder {
    instance: Arc<Instance>,
    physical_device: PhysicalDevice,
    allocation_callbacks: Option<AllocationCallbacks<'static>>,
    // TODO: pNext chains for features
    // TODO: queue descriptions
}

impl DeviceBuilder {
    pub fn new(physical_device: PhysicalDevice, instance: Arc<Instance>) -> DeviceBuilder {
        Self {
            physical_device,
            allocation_callbacks: None,
            instance,
        }
    }

    pub fn allocation_callbacks(
        mut self,
        allocation_callbacks: AllocationCallbacks<'static>,
    ) -> Self {
        self.allocation_callbacks.replace(allocation_callbacks);
        self
    }

    pub fn build(mut self) -> crate::Result<Device> {
        // TODO: custom queue setup
        // (index, priorities)
        let queue_descriptions = self
            .physical_device
            .queue_families
            .iter()
            .enumerate()
            .map(|(index, _)| (index, [1.]))
            .collect::<Vec<_>>();

        let queue_create_infos = queue_descriptions
            .iter()
            .map(|(index, priorities)| {
                vk::DeviceQueueCreateInfo::default()
                    .queue_family_index(*index as u32)
                    .queue_priorities(priorities)
            })
            .collect::<Vec<_>>();
        let extensions_to_enable = self
            .physical_device
            .extensions_to_enable
            .iter()
            .map(|e| cow_to_c_cow(e.clone()))
            .collect::<Vec<_>>();

        let mut extensions_to_enable = extensions_to_enable
            .iter()
            .map(|e| e.as_ptr())
            .collect::<Vec<_>>();
        if self.physical_device.surface.is_some()
            || self.physical_device.defer_surface_initialization
        {
            extensions_to_enable.push(vk::KHR_SWAPCHAIN_NAME.as_ptr());
        }

        let mut device_create_info = vk::DeviceCreateInfo::default()
            .queue_create_infos(&queue_create_infos)
            .enabled_extension_names(&extensions_to_enable);

        let requested_features_chain = &mut self.physical_device.requested_features_chain;

        let mut features2 =
            vk::PhysicalDeviceFeatures2::default().features(self.physical_device.features);

        if self.instance.instance_version >= vk::API_VERSION_1_1
            || self.physical_device.properties2_ext_enabled
        {
            device_create_info = device_create_info.push_next(&mut features2);

            for node in requested_features_chain.nodes.iter_mut() {
                match node {
                    VulkanPhysicalDeviceFeature2::PhysicalDeviceVulkan11(f) => {
                        device_create_info = device_create_info.push_next(f)
                    }
                    VulkanPhysicalDeviceFeature2::PhysicalDeviceVulkan12(f) => {
                        device_create_info = device_create_info.push_next(f)
                    }
                    VulkanPhysicalDeviceFeature2::PhysicalDeviceVulkan13(f) => {
                        device_create_info = device_create_info.push_next(f)
                    }
                }
            }
        }

        let device = unsafe {
            self.instance.instance.create_device(
                self.physical_device.physical_device,
                &device_create_info,
                self.allocation_callbacks.as_ref(),
            )
        }?;

        let physical_device = self.physical_device;

        let surface = physical_device.surface;
        let allocation_callbacks = self.allocation_callbacks;

        Ok(Device {
            device,
            surface,
            surface_instance: self.instance.surface_instance.clone(),
            physical_device,
            allocation_callbacks,
        })
    }
}

pub struct Device {
    device: ash::Device,
    physical_device: PhysicalDevice,
    surface: Option<vk::SurfaceKHR>,
    surface_instance: Option<khr::surface::Instance>,
    allocation_callbacks: Option<AllocationCallbacks<'static>>,
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Ord)]
pub enum QueueType {
    Present,
    Graphics,
    Compute,
    Transfer,
}

impl Device {
    pub fn device(&self) -> &ash::Device {
        &self.device
    }

    pub fn physical_device(&self) -> vk::PhysicalDevice {
        self.physical_device.physical_device
    }

    pub fn get_queue(&self, queue: QueueType) -> crate::Result<(usize, vk::Queue)> {
        let index = match queue {
            QueueType::Present => get_present_queue_index(
                &self.surface_instance,
                self.physical_device.physical_device,
                self.surface,
                &self.physical_device.queue_families,
            )
            .ok_or(crate::QueueError::PresentUnavailable),
            QueueType::Graphics => get_first_queue_index(
                &self.physical_device.queue_families,
                vk::QueueFlags::GRAPHICS,
            )
            .ok_or(crate::QueueError::GraphicsUnavailable),
            QueueType::Compute => get_separate_queue_index(
                &self.physical_device.queue_families,
                vk::QueueFlags::COMPUTE,
                vk::QueueFlags::TRANSFER,
            )
            .ok_or(crate::QueueError::ComputeUnavailable),
            QueueType::Transfer => get_separate_queue_index(
                &self.physical_device.queue_families,
                vk::QueueFlags::TRANSFER,
                vk::QueueFlags::COMPUTE,
            )
            .ok_or(crate::QueueError::TransferUnavailable),
        }?;

        let info = vk::DeviceQueueInfo2::default()
            .queue_family_index(index as _)
            .queue_index(0);

        Ok((index, unsafe { self.device.get_device_queue2(&info) }))
    }

    pub fn get_dedicated_queue(&self, queue: QueueType) -> crate::Result<vk::Queue> {
        let index = match queue {
            QueueType::Compute => get_dedicated_queue_index(
                &self.physical_device.queue_families,
                vk::QueueFlags::COMPUTE,
                vk::QueueFlags::TRANSFER,
            )
            .ok_or(crate::QueueError::ComputeUnavailable),
            QueueType::Transfer => get_dedicated_queue_index(
                &self.physical_device.queue_families,
                vk::QueueFlags::TRANSFER,
                vk::QueueFlags::COMPUTE,
            )
            .ok_or(crate::QueueError::TransferUnavailable),
            _ => return Err(crate::QueueError::InvalidQueueFamilyIndex.into()),
        }?;

        let info = vk::DeviceQueueInfo2::default()
            .queue_family_index(index as _)
            .queue_index(0);

        Ok(unsafe { self.device.get_device_queue2(&info) })
    }
    
    pub fn destroy(&self) {
        unsafe {
            self.device
                .destroy_device(self.allocation_callbacks.as_ref());
        }
    }
}

impl AsRef<ash::Device> for Device {
    fn as_ref(&self) -> &ash::Device {
        &self.device
    }
}

impl Deref for Device {
    type Target = ash::Device;

    fn deref(&self) -> &Self::Target {
        &self.device
    }
}