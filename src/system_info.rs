use std::fmt::{Debug, Formatter};
use vulkanalia::loader::{LIBRARY, LibloadingLoader};
use vulkanalia::vk::{EntryV1_0, EntryV1_1};
use vulkanalia::{Entry, vk};

pub const VALIDATION_LAYER_NAME: vk::ExtensionName =
    vk::ExtensionName::from_bytes(b"VK_LAYER_KHRONOS_validation");
pub const DEBUG_UTILS_EXT_NAME: vk::ExtensionName = vk::EXT_DEBUG_UTILS_EXTENSION.name;

pub struct SystemInfo {
    pub available_layers: Vec<vk::LayerProperties>,
    pub available_extensions: Vec<vk::ExtensionProperties>,
    pub validation_layers_available: bool,
    pub debug_utils_available: bool,
    pub instance_api_version: u32,
    pub(crate) entry: Entry,
}

impl Debug for SystemInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SystemInfo")
            .field("available_layers", &self.available_layers)
            .field("available_extensions", &self.available_extensions)
            .field(
                "validation_layers_available",
                &self.validation_layers_available,
            )
            .field("debug_utils_available", &self.debug_utils_available)
            .field("instance_api_version", &self.instance_api_version)
            .finish()
    }
}

impl SystemInfo {
    #[cfg_attr(feature = "enable_tracing", tracing::instrument)]
    pub fn get_system_info() -> crate::Result<Self> {
        #[cfg(feature = "enable_tracing")]
        tracing::trace!("Loading entry...");
        let loader = unsafe { LibloadingLoader::new(LIBRARY) }?;
        let entry = unsafe { Entry::new(loader).unwrap() };
        #[cfg(feature = "enable_tracing")]
        tracing::trace!("Entry loaded.");
        let mut validation_layers_available = false;
        let mut debug_utils_available = false;

        let available_layers = unsafe { entry.enumerate_instance_layer_properties() }?;

        for layer in &available_layers {
            if layer.layer_name.to_string_lossy() == VALIDATION_LAYER_NAME.to_string_lossy() {
                validation_layers_available = true;
                break;
            }
        }

        let mut available_extensions =
            unsafe { entry.enumerate_instance_extension_properties(None) }?;

        for ext in &available_extensions {
            if ext.extension_name == DEBUG_UTILS_EXT_NAME {
                debug_utils_available = true;
            }
        }

        for layer in &available_layers {
            let layer_extensions = unsafe {
                entry.enumerate_instance_extension_properties(Some(layer.layer_name.as_bytes()))
            }?;

            available_extensions.extend_from_slice(&layer_extensions);

            for ext in &layer_extensions {
                if ext.extension_name == DEBUG_UTILS_EXT_NAME {
                    debug_utils_available = true;
                }
            }
        }

        #[cfg(feature = "enable_tracing")]
        tracing::trace!(validation_layers_available, debug_utils_available);

        let instance_api_version = unsafe { entry.enumerate_instance_version() }?;

        Ok(Self {
            available_layers,
            available_extensions,
            debug_utils_available,
            validation_layers_available,
            instance_api_version,
            entry,
        })
    }

    pub fn is_extension_available(&self, extension: &vk::ExtensionName) -> crate::Result<bool> {
        for ext in &self.available_extensions {
            if ext.extension_name == *extension {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn are_extensions_available(
        &self,
        extensions: &Vec<vk::ExtensionName>,
    ) -> crate::Result<bool> {
        let mut all_found = true;
        for ext in extensions {
            let found = self.is_extension_available(ext)?;
            if !found {
                all_found = false;
            }
        }

        Ok(all_found)
    }

    pub fn is_layer_available(&self, layer: vk::ExtensionName) -> crate::Result<bool> {
        for ext in &self.available_layers {
            if ext.layer_name.to_string_lossy() == layer.to_string_lossy() {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn are_layers_available<'a, I: IntoIterator<Item = vk::ExtensionName>>(
        &self,
        layers: I,
    ) -> crate::Result<bool> {
        let mut all_found = true;
        for ext in layers {
            let found = self.is_layer_available(ext)?;
            if !found {
                all_found = false;
            }
        }

        Ok(all_found)
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn test() {}
}
