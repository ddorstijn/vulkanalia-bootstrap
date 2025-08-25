use crate::system_info::{DEBUG_UTILS_EXT_NAME, SystemInfo, VALIDATION_LAYER_NAME};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle};
use std::borrow::Cow;
use std::ffi;
use std::ffi::c_void;
use std::fmt::Debug;
use std::sync::Arc;
use vulkanalia::vk::{
    self, EntryV1_1, ExtDebugUtilsExtension, HasBuilder, InstanceV1_0, KhrSurfaceExtension,
};
use vulkanalia::vk::{AllocationCallbacks, DebugUtilsMessengerEXT};
use vulkanalia::{Version, window as vk_window};

pub trait WindowTraits: HasDisplayHandle + HasWindowHandle + Debug {}
impl<T> WindowTraits for T where T: HasDisplayHandle + HasWindowHandle + Debug {}

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    unsafe {
        let callback_data = *p_callback_data;
        let message_id_number = callback_data.message_id_number;

        let message_id_name = if callback_data.message_id_name.is_null() {
            Cow::from("")
        } else {
            ffi::CStr::from_ptr(callback_data.message_id_name).to_string_lossy()
        };

        let message = if callback_data.message.is_null() {
            Cow::from("")
        } else {
            ffi::CStr::from_ptr(callback_data.message).to_string_lossy()
        };

        println!(
            "{message_severity:?}:\n{message_type:?} [{message_id_name} ({message_id_number})] : {message}\n",
        );

        vk::FALSE
    }
}

#[derive(Debug)]
pub struct DebugUserData(*mut c_void);

impl Default for DebugUserData {
    fn default() -> Self {
        Self(std::ptr::null_mut())
    }
}

impl DebugUserData {
    /// Caller must ensure that data pointer points to valid memory.
    pub unsafe fn new(data: *mut c_void) -> Self {
        Self(data)
    }
}

impl DebugUserData {
    pub fn into_inner(self) -> *mut c_void {
        self.0
    }
}

#[derive(Debug)]
pub struct InstanceBuilder {
    // VkApplicationInfo
    app_name: String,
    engine_name: String,
    application_version: Version,
    engine_version: Version,
    minimum_instance_version: Version,
    required_instance_version: Version,

    // VkInstanceCreateInfo
    layers: Vec<vk::ExtensionName>,
    extensions: Vec<vk::ExtensionName>,
    flags: vk::InstanceCreateFlags,

    // debug callback
    debug_callback: vk::PFN_vkDebugUtilsMessengerCallbackEXT,
    debug_message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    debug_message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    debug_user_data: DebugUserData,

    // validation checks
    disabled_validation_checks: Vec<vk::ValidationCheckEXT>,
    enabled_validation_features: Vec<vk::ValidationFeatureEnableEXT>,
    disabled_validation_features: Vec<vk::ValidationFeatureDisableEXT>,

    allocation_callbacks: Option<vk::AllocationCallbacks>,

    request_validation_layers: bool,
    enable_validation_layers: bool,
    // TODO: make typesafe
    use_debug_messenger: bool,
    headless_context: bool,

    window: Option<Arc<dyn WindowTraits>>,
}

impl InstanceBuilder {
    pub fn new(window: Option<Arc<dyn WindowTraits>>) -> Self {
        Self {
            app_name: "".to_string(),
            engine_name: "".to_string(),
            application_version: Version::new(0, 0, 0),
            engine_version: Version::new(0, 0, 0),
            minimum_instance_version: Version::V1_0_0,
            required_instance_version: Version::V1_0_0,
            layers: vec![],
            extensions: vec![],
            flags: Default::default(),
            debug_callback: None,
            debug_message_severity: vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
            debug_message_type: vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION
                | vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE,
            debug_user_data: Default::default(),
            disabled_validation_checks: vec![],
            enabled_validation_features: vec![],
            disabled_validation_features: vec![],
            allocation_callbacks: None,
            request_validation_layers: false,
            enable_validation_layers: false,
            use_debug_messenger: false,
            headless_context: false,
            window,
        }
    }

    pub fn app_name(mut self, app_name: impl Into<String>) -> Self {
        self.app_name = app_name.into();
        self
    }

    pub fn engine_name(mut self, engine_name: impl Into<String>) -> Self {
        self.engine_name = engine_name.into();
        self
    }

    pub fn app_version(mut self, version: Version) -> Self {
        self.application_version = version;
        self
    }

    pub fn engine_version(mut self, version: Version) -> Self {
        self.engine_version = version;
        self
    }

    pub fn require_api_version(mut self, version: Version) -> Self {
        self.required_instance_version = version;
        self
    }

    pub fn minimum_instance_version(mut self, version: Version) -> Self {
        self.minimum_instance_version = version;
        self
    }

    pub fn enable_layer(mut self, layer: vk::ExtensionName) -> Self {
        self.layers.push(layer.into());
        self
    }

    pub fn enable_extension(mut self, extension: vk::ExtensionName) -> Self {
        self.extensions.push(extension);
        self
    }

    pub fn enable_validation_layers(mut self, enable: bool) -> Self {
        self.enable_validation_layers = enable;
        self
    }

    pub fn request_validation_layers(mut self, request: bool) -> Self {
        self.request_validation_layers = request;
        self
    }

    pub fn use_default_debug_messenger(mut self) -> Self {
        self.use_debug_messenger = true;
        self.debug_callback = Some(vulkan_debug_callback);
        self
    }

    #[cfg(feature = "enable_tracing")]
    pub fn use_default_tracing_messenger(mut self) -> Self {
        self.use_debug_messenger = true;
        self.debug_callback = Some(crate::tracing::vulkan_tracing_callback);
        self
    }

    pub fn set_debug_messenger(
        mut self,
        callback: vk::PFN_vkDebugUtilsMessengerCallbackEXT,
    ) -> Self {
        self.use_debug_messenger = true;
        self.debug_callback = callback;
        self
    }

    pub fn debug_user_data(mut self, debug_user_data: DebugUserData) -> Self {
        self.debug_user_data = debug_user_data;
        self
    }

    pub fn headless(mut self, headless: bool) -> Self {
        self.headless_context = headless;
        self
    }

    pub fn debug_messenger_severity(
        mut self,
        severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    ) -> Self {
        self.debug_message_severity = severity;
        self
    }

    pub fn add_debug_messenger_severity(
        mut self,
        severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    ) -> Self {
        self.debug_message_severity |= severity;
        self
    }

    pub fn debug_messenger_type(mut self, message_type: vk::DebugUtilsMessageTypeFlagsEXT) -> Self {
        self.debug_message_type = message_type;
        self
    }

    pub fn add_debug_messenger_type(
        mut self,
        message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    ) -> Self {
        self.debug_message_type |= message_type;
        self
    }

    #[cfg_attr(feature = "enable_tracing", tracing::instrument(skip(self)))]
    pub fn build(self) -> crate::Result<Arc<Instance>> {
        let system_info = SystemInfo::get_system_info()?;

        let instance_version = {
            if self.minimum_instance_version > Version::V1_0_0
                || self.required_instance_version > Version::V1_0_0
            {
                let version = unsafe { system_info.entry.enumerate_instance_version() }
                    .map_or(Version::V1_0_0, Version::from);

                if version < self.minimum_instance_version
                    || (self.minimum_instance_version == Version::V1_0_0
                        && version < self.required_instance_version)
                {
                    return match self
                        .required_instance_version
                        .max(self.minimum_instance_version)
                        .minor
                    {
                        3 => Err(crate::InstanceError::VulkanVersion13Unavailable.into()),
                        2 => Err(crate::InstanceError::VulkanVersion12Unavailable.into()),
                        1 => Err(crate::InstanceError::VulkanVersion11Unavailable.into()),
                        minor => Err(crate::InstanceError::VulkanVersionUnavailable(format!(
                            "1.{minor}"
                        ))
                        .into()),
                    };
                } else {
                    version
                }
            } else {
                Version::V1_0_0
            }
        };

        #[cfg(feature = "enable_tracing")]
        {
            tracing::info!(
                "Instance version: {}.{}.{}",
                instance_version.major,
                instance_version.minor,
                instance_version.patch
            );
        }

        let api_version = if instance_version < Version::V1_1_0
            || self.required_instance_version < self.minimum_instance_version
        {
            instance_version
        } else {
            self.required_instance_version
                .max(self.minimum_instance_version)
        };
        #[cfg(feature = "enable_tracing")]
        {
            tracing::info!("api_version: {}", api_version);
        }

        let app_name = self.app_name;
        let engine_name = self.engine_name;

        let app_info = vk::ApplicationInfo {
            application_name: app_name.as_bytes().as_ptr() as _,
            application_version: self.application_version.into(),
            engine_name: engine_name.as_bytes().as_ptr() as _,
            engine_version: self.engine_version.into(),
            api_version: api_version.into(),
            ..Default::default()
        };

        #[cfg(feature = "enable_tracing")]
        {
            tracing::info!("Creating vkInstance with application info...");
            tracing::debug!(
                r#"
Application info: {{
    name: {:?},
    version: {}.{}.{},
    engine_name: {:?},
    engine_version: {}.{}.{},
    api_version: {}.{}.{},
}}
            "#,
                app_name,
                self.application_version.major,
                self.application_version.minor,
                self.application_version.patch,
                engine_name,
                self.engine_version.major,
                self.engine_version.minor,
                self.engine_version.patch,
                api_version.major,
                api_version.minor,
                api_version.patch,
            )
        }

        let mut enabled_extensions: Vec<vk::ExtensionName> = vec![];
        let mut enabled_layers: Vec<vk::ExtensionName> = vec![];

        enabled_extensions.extend_from_slice(self.extensions.as_slice());

        if self.debug_callback.is_some()
            && self.use_debug_messenger
            && system_info.debug_utils_available
        {
            enabled_extensions.push(DEBUG_UTILS_EXT_NAME);
        }

        let properties2_ext_enabled = api_version < Version::V1_1_0
            && system_info
                .is_extension_available(&vk::KHR_GET_PHYSICAL_DEVICE_PROPERTIES2_EXTENSION.name)?;

        if properties2_ext_enabled {
            enabled_extensions.push(vk::KHR_GET_PHYSICAL_DEVICE_PROPERTIES2_EXTENSION.name);
        }

        #[cfg(feature = "portability")]
        let portability_enumeration_support =
            system_info.is_extension_available(&vk::KHR_PORTABILITY_ENUMERATION_EXTENSION.name)?;
        #[cfg(feature = "portability")]
        if portability_enumeration_support {
            enabled_extensions.push(vk::KHR_PORTABILITY_ENUMERATION_EXTENSION.name);
        }

        if !self.headless_context {
            if let Some(window) = self.window.clone() {
                let surface_extensions: Vec<vk::ExtensionName> =
                    vk_window::get_required_instance_extensions(&window)
                        .into_iter()
                        .map(|ext| **ext)
                        .collect();

                if !system_info.are_extensions_available(&surface_extensions)? {
                    return Err(crate::InstanceError::WindowingExtensionsNotPresent(
                        surface_extensions,
                    )
                    .into());
                };

                enabled_extensions.extend_from_slice(&surface_extensions);
            }
        }

        #[cfg(feature = "enable_tracing")]
        tracing::trace!(?enabled_extensions);

        let all_extensions_supported = system_info.are_extensions_available(&enabled_extensions)?;
        if !all_extensions_supported {
            return Err(
                crate::InstanceError::RequestedExtensionsNotPresent(enabled_extensions).into(),
            );
        };

        enabled_layers.extend_from_slice(&self.layers);

        if self.enable_validation_layers
            || (self.request_validation_layers && system_info.validation_layers_available)
        {
            enabled_layers.push(VALIDATION_LAYER_NAME)
        };

        let all_layers_supported = system_info.are_layers_available(self.layers)?;

        if !all_layers_supported {
            return Err(crate::InstanceError::RequestedLayersNotPresent(enabled_layers).into());
        };

        let instance_create_flags = if cfg!(feature = "portability") {
            self.flags | vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR
        } else {
            self.flags
        };

        let enabled_extension_ptr = enabled_extensions
            .iter()
            .map(|e| e.as_ptr())
            .collect::<Vec<_>>();

        let enabled_layers_ptr = enabled_layers
            .iter()
            .map(|e| e.as_ptr())
            .collect::<Vec<_>>();

        let mut instance_create_info = vk::InstanceCreateInfo::builder()
            .flags(instance_create_flags)
            .application_info(&app_info)
            .enabled_extension_names(&enabled_extension_ptr)
            .enabled_layer_names(&enabled_layers_ptr);

        let mut features = vk::ValidationFeaturesEXT::builder()
            .disabled_validation_features(&self.disabled_validation_features)
            .enabled_validation_features(&self.enabled_validation_features);

        if !self.enabled_validation_features.is_empty()
            || !self.disabled_validation_features.is_empty()
        {
            instance_create_info = instance_create_info.push_next(&mut features);
        };

        let mut checks = vk::ValidationFlagsEXT::builder();
        if !self.disabled_validation_checks.is_empty() {
            checks = checks.disabled_validation_checks(&self.disabled_validation_checks);

            instance_create_info = instance_create_info.push_next(&mut checks);
        };

        let instance = unsafe {
            system_info
                .entry
                .create_instance(&instance_create_info, self.allocation_callbacks.as_ref())
        }
        .map_err(|_| crate::InstanceError::FailedCreateInstance)?;

        #[cfg(feature = "enable_tracing")]
        tracing::info!("Created vkInstance");

        let mut debug_messenger = None;
        let mut debug_user_data = self.debug_user_data.into_inner();

        if self.use_debug_messenger {
            let messenger_create_info = vk::DebugUtilsMessengerCreateInfoEXT::builder()
                .message_severity(self.debug_message_severity)
                .message_type(self.debug_message_type)
                .user_callback(self.debug_callback)
                .user_data(&mut debug_user_data);

            #[cfg(feature = "enable_tracing")]
            tracing::trace!(?self.debug_callback, "Using debug messenger");

            let messenger =
                unsafe { instance.create_debug_utils_messenger_ext(&messenger_create_info, None) }?;

            debug_messenger.replace(messenger);
        };

        let mut surface = None;
        if let Some(window) = self.window.clone() {
            surface = Some(unsafe { vk_window::create_surface(&instance, &window, &window)? });
            #[cfg(feature = "enable_tracing")]
            tracing::info!("Created vkSurfaceKhr")
        };

        Ok(Arc::new(Instance {
            instance,
            surface,
            allocation_callbacks: self.allocation_callbacks,
            instance_version,
            api_version,
            properties2_ext_enabled,
            debug_messenger,
            _system_info: system_info,
        }))
    }
}

pub struct Instance {
    pub(crate) instance: vulkanalia::Instance,
    pub(crate) allocation_callbacks: Option<AllocationCallbacks>,
    pub(crate) surface: Option<vk::SurfaceKHR>,
    pub(crate) instance_version: Version,
    pub api_version: Version,
    pub(crate) properties2_ext_enabled: bool,
    pub(crate) debug_messenger: Option<DebugUtilsMessengerEXT>,
    _system_info: SystemInfo,
}

impl Instance {
    pub fn destroy(&self) {
        unsafe {
            if let Some(debug_messenger) = self.debug_messenger {
                self.instance.destroy_debug_utils_messenger_ext(
                    debug_messenger,
                    self.allocation_callbacks.as_ref(),
                );
            }
            if let Some(surface) = self.surface {
                self.instance
                    .destroy_surface_khr(surface, self.allocation_callbacks.as_ref());
            }
            self.instance
                .destroy_instance(self.allocation_callbacks.as_ref());
        }
    }
}

impl AsRef<vulkanalia::Instance> for Instance {
    fn as_ref(&self) -> &vulkanalia::Instance {
        &self.instance
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn compiles() {}
}
