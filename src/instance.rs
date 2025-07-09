use crate::system_info::{DEBUG_UTILS_EXT_NAME, SystemInfo, VALIDATION_LAYER_NAME};
use ash::ext::debug_utils;
use ash::vk::{AllocationCallbacks, DebugUtilsMessengerEXT, api_version_minor};
use ash::{khr, vk};
use raw_window_handle::{DisplayHandle, RawDisplayHandle, RawWindowHandle, WindowHandle};
use std::borrow::Cow;
use std::ffi;
use std::ffi::{CStr, CString, c_char, c_void};
use std::ops::Not;

unsafe extern "system" fn vulkan_debug_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT<'_>,
    _user_data: *mut std::os::raw::c_void,
) -> vk::Bool32 {
    unsafe {
        let callback_data = *p_callback_data;
        let message_id_number = callback_data.message_id_number;

        let message_id_name = if callback_data.p_message_id_name.is_null() {
            Cow::from("")
        } else {
            ffi::CStr::from_ptr(callback_data.p_message_id_name).to_string_lossy()
        };

        let message = if callback_data.p_message.is_null() {
            Cow::from("")
        } else {
            ffi::CStr::from_ptr(callback_data.p_message).to_string_lossy()
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
pub struct InstanceBuilder<'a> {
    // VkApplicationInfo
    app_name: String,
    engine_name: String,
    application_version: u32,
    engine_version: u32,
    minimum_instance_version: u32,
    required_instance_version: u32,

    // VkInstanceCreateInfo
    layers: Vec<String>,
    extensions: Vec<Cow<'a, str>>,
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

    allocation_callbacks: Option<vk::AllocationCallbacks<'static>>,

    request_validation_layers: bool,
    enable_validation_layers: bool,
    // TODO: make typesafe
    use_debug_messenger: bool,
    headless_context: bool,

    window_handle: Option<RawWindowHandle>,
    display_handle: Option<RawDisplayHandle>,
}

impl<'a> InstanceBuilder<'a> {
    pub fn new(window_display_handle: Option<(WindowHandle, DisplayHandle)>) -> Self {
        let (window_handle, display_handle) = window_display_handle.unzip();
        Self {
            app_name: "".to_string(),
            engine_name: "".to_string(),
            application_version: 0,
            engine_version: 0,
            minimum_instance_version: 0,
            required_instance_version: vk::API_VERSION_1_0,
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
            display_handle: display_handle.map(|h| h.as_raw()),
            window_handle: window_handle.map(|h| h.as_raw()),
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

    pub fn app_version(mut self, version: u32) -> Self {
        self.application_version = version;
        self
    }

    pub fn engine_version(mut self, version: u32) -> Self {
        self.engine_version = version;
        self
    }

    pub fn require_api_version(mut self, version: u32) -> Self {
        self.required_instance_version = version;
        self
    }

    pub fn minimum_instance_version(mut self, version: u32) -> Self {
        self.minimum_instance_version = version;
        self
    }

    pub fn enable_layer(mut self, layer: impl Into<String>) -> Self {
        self.layers.push(layer.into());
        self
    }

    pub fn enable_extension(mut self, extension: impl Into<Cow<'a, str>>) -> Self {
        self.extensions.push(extension.into());
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
    pub fn build(self) -> crate::Result<Instance> {
        let system_info = SystemInfo::get_system_info()?;

        let instance_version = {
            if self.minimum_instance_version > vk::API_VERSION_1_0
                || self.required_instance_version > vk::API_VERSION_1_0
            {
                let version = unsafe { system_info.entry.try_enumerate_instance_version() }?;

                let version = version.unwrap_or(vk::API_VERSION_1_0);

                if version < self.minimum_instance_version
                    || (self.minimum_instance_version == 0
                        && version < self.required_instance_version)
                {
                    return match api_version_minor(
                        self.required_instance_version
                            .max(self.minimum_instance_version),
                    ) {
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
                vk::API_VERSION_1_0
            }
        };

        #[cfg(feature = "enable_tracing")]
        {
            tracing::info!(
                "Instance version: {}.{}.{}",
                vk::api_version_major(instance_version),
                vk::api_version_minor(instance_version),
                vk::api_version_patch(instance_version)
            );
        }

        let api_version = if instance_version < vk::API_VERSION_1_1
            || self.required_instance_version < self.minimum_instance_version
        {
            instance_version
        } else {
            self.required_instance_version
                .max(self.minimum_instance_version)
        };
        #[cfg(feature = "enable_tracing")]
        {
            use crate::version::Version;
            let version = Version::new(api_version);
            tracing::info!("api_version: {}", version);
        }

        let app_name = CString::new(self.app_name).map_err(anyhow::Error::msg)?;
        let engine_name = CString::new(self.engine_name).map_err(anyhow::Error::msg)?;

        let app_info = vk::ApplicationInfo::default()
            .application_name(&app_name)
            .application_version(self.application_version)
            .engine_name(&engine_name)
            .engine_version(self.engine_version)
            .api_version(api_version);

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
                vk::api_version_major(self.application_version),
                vk::api_version_minor(self.application_version),
                vk::api_version_patch(self.application_version),
                engine_name,
                vk::api_version_major(self.engine_version),
                vk::api_version_minor(self.engine_version),
                vk::api_version_patch(self.engine_version),
                vk::api_version_major(api_version),
                vk::api_version_minor(api_version),
                vk::api_version_patch(api_version),
            )
        }

        let mut enabled_extensions: Vec<*const c_char> = vec![];
        let mut enabled_layers: Vec<*const c_char> = vec![];

        let extensions = self
            .extensions
            .into_iter()
            .map(|s| CString::new(s.to_string()).expect("Could not create CString"))
            .collect::<Vec<_>>();

        let extensions_ptrs = extensions.iter().map(|e| e.as_ptr()).collect::<Vec<_>>();

        enabled_extensions.extend_from_slice(&extensions_ptrs);

        if self.debug_callback.is_some()
            && self.use_debug_messenger
            && system_info.debug_utils_available
        {
            enabled_extensions.push(DEBUG_UTILS_EXT_NAME.as_ptr());
        }

        let properties2_ext_enabled = api_version < vk::API_VERSION_1_1
            && system_info.is_extension_available(vk::KHR_GET_PHYSICAL_DEVICE_PROPERTIES2_NAME)?;

        if properties2_ext_enabled {
            enabled_extensions.push(vk::KHR_GET_PHYSICAL_DEVICE_PROPERTIES2_NAME.as_ptr());
        }

        #[cfg(feature = "portability")]
        let portability_enumeration_support =
            system_info.is_extension_available(vk::KHR_PORTABILITY_ENUMERATION_NAME)?;
        #[cfg(feature = "portability")]
        if portability_enumeration_support {
            enabled_extensions.push(vk::KHR_PORTABILITY_ENUMERATION_NAME.as_ptr());
        }

        if !self.headless_context {
            if let Some(display_handle) = self.display_handle {
                let surface_extensions_raw =
                    ash_window::enumerate_required_extensions(display_handle)?;
                let surface_extensions = surface_extensions_raw
                    .iter()
                    .map(|p| unsafe { CStr::from_ptr(*p) })
                    .collect::<Vec<_>>();
                let windowing_extensions = surface_extensions
                    .iter()
                    .map(|s| s.to_str().unwrap().to_string())
                    .collect::<Vec<_>>();
                if !system_info.are_extensions_available(surface_extensions)? {
                    return Err(crate::InstanceError::WindowingExtensionsNotPresent(
                        windowing_extensions,
                    )
                    .into());
                };

                enabled_extensions.extend_from_slice(surface_extensions_raw);
            }
        }

        let cstr_enabled_extensions = enabled_extensions
            .iter()
            .map(|p| unsafe { CStr::from_ptr(*p) })
            .collect::<Vec<_>>();

        #[cfg(feature = "enable_tracing")]
        tracing::trace!(?cstr_enabled_extensions);

        let all_extensions_supported =
            system_info.are_extensions_available(cstr_enabled_extensions)?;
        if !all_extensions_supported {
            let string_enabled_extensions = enabled_extensions
                .iter()
                .map(|p| unsafe { CStr::from_ptr(*p) }.to_str().unwrap().to_string())
                .collect::<Vec<_>>();

            return Err(crate::InstanceError::RequestedExtensionsNotPresent(
                string_enabled_extensions,
            )
            .into());
        };

        let layers = self
            .layers
            .into_iter()
            .map(|s| CString::new(s).expect("Could not create CString"))
            .collect::<Vec<_>>();

        let layers_ptrs = layers.iter().map(|e| e.as_ptr()).collect::<Vec<_>>();

        enabled_layers.extend_from_slice(&layers_ptrs);

        if self.enable_validation_layers
            || (self.request_validation_layers && system_info.validation_layers_available)
        {
            enabled_layers.push(VALIDATION_LAYER_NAME.as_ptr())
        };

        let all_layers_supported =
            system_info.are_layers_available(layers.iter().map(|s| s.as_c_str()))?;

        if !all_layers_supported {
            let enabled_layers_str = enabled_layers
                .iter()
                .map(|p| unsafe { CStr::from_ptr(*p) }.to_str().unwrap().to_string())
                .collect::<Vec<_>>();
            return Err(crate::InstanceError::RequestedLayersNotPresent(enabled_layers_str).into());
        };

        let mut messenger_create_info = vk::DebugUtilsMessengerCreateInfoEXT::default();
        if self.use_debug_messenger {
            messenger_create_info = messenger_create_info
                .message_severity(self.debug_message_severity)
                .message_type(self.debug_message_type)
                .pfn_user_callback(self.debug_callback)
                .user_data(self.debug_user_data.into_inner());

            #[cfg(feature = "enable_tracing")]
            tracing::trace!(?self.debug_callback, "Using debug messenger");
        };

        let instance_create_flags = if cfg!(feature = "portability") {
            self.flags | vk::InstanceCreateFlags::ENUMERATE_PORTABILITY_KHR
        } else {
            self.flags
        };

        let mut instance_create_info = vk::InstanceCreateInfo::default()
            .flags(instance_create_flags)
            .application_info(&app_info)
            .enabled_extension_names(&enabled_extensions)
            .enabled_layer_names(&enabled_layers);

        let mut features = vk::ValidationFeaturesEXT::default();

        if !self.enabled_validation_features.is_empty()
            || !self.disabled_validation_features.is_empty()
        {
            features = features
                .enabled_validation_features(&self.enabled_validation_features)
                .disabled_validation_features(&self.disabled_validation_features);

            instance_create_info = instance_create_info.push_next(&mut features);
        };

        let mut checks = vk::ValidationFlagsEXT::default();

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

        let mut debug_loader = None;
        let mut debug_messenger = None;

        if self.use_debug_messenger {
            let loader = debug_utils::Instance::new(&system_info.entry, &instance);
            let messenger = unsafe {
                loader.create_debug_utils_messenger(
                    &messenger_create_info,
                    self.allocation_callbacks.as_ref(),
                )
            }?;

            debug_loader.replace(loader);
            debug_messenger.replace(messenger);
        };

        let surface_instance = self
            .headless_context
            .not()
            .then(|| khr::surface::Instance::new(&system_info.entry, &instance));
        let mut surface = None;
        if let Some((window_handle, display_handle)) = self.window_handle.zip(self.display_handle) {
            if surface_instance.is_some() {
                surface = Some(unsafe {
                    ash_window::create_surface(
                        &system_info.entry,
                        &instance,
                        display_handle,
                        window_handle,
                        None,
                    )?
                });
                #[cfg(feature = "enable_tracing")]
                tracing::info!("Created vkSurfaceKhr")
            }
        };

        Ok(Instance {
            instance,
            surface_instance,
            surface,
            allocation_callbacks: self.allocation_callbacks,
            instance_version,
            api_version,
            properties2_ext_enabled,
            debug_loader,
            debug_messenger,
            _system_info: system_info,
        })
    }
}

pub struct Instance {
    pub(crate) instance: ash::Instance,
    pub(crate) allocation_callbacks: Option<AllocationCallbacks<'static>>,
    pub(crate) surface_instance: Option<khr::surface::Instance>,
    pub(crate) surface: Option<vk::SurfaceKHR>,
    pub(crate) instance_version: u32,
    pub api_version: u32,
    pub(crate) properties2_ext_enabled: bool,
    pub(crate) debug_loader: Option<debug_utils::Instance>,
    pub(crate) debug_messenger: Option<DebugUtilsMessengerEXT>,
    _system_info: SystemInfo,
}

impl Instance {
    pub fn destroy(&self) {
        unsafe {
            if let Some((debug_messenger, debug_loader)) = self
                .debug_messenger
                .as_ref()
                .zip(self.debug_loader.as_ref())
            {
                debug_loader.destroy_debug_utils_messenger(
                    *debug_messenger,
                    self.allocation_callbacks.as_ref(),
                );
            }
            if let Some((surface_instance, surface)) =
                self.surface_instance.as_ref().zip(self.surface)
            {
                surface_instance.destroy_surface(surface, self.allocation_callbacks.as_ref());
            }
            self.instance
                .destroy_instance(self.allocation_callbacks.as_ref());
        }
    }
}

impl AsRef<ash::Instance> for Instance {
    fn as_ref(&self) -> &ash::Instance {
        &self.instance
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn compiles() {}
}
