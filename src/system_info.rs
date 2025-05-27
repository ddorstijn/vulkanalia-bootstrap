use ash::{Entry, vk};
use std::ffi::CStr;
use std::fmt::{Debug, Formatter};

pub const VALIDATION_LAYER_NAME: &CStr = c"VK_LAYER_KHRONOS_validation";
pub const DEBUG_UTILS_EXT_NAME: &CStr = vk::EXT_DEBUG_UTILS_NAME;

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
    #[cfg_attr(feature = "tracing", tracing::instrument)]
    pub fn get_system_info() -> crate::Result<Self> {
        #[cfg(feature = "tracing")]
        tracing::trace!("Loading entry...");
        let entry = unsafe { Entry::load() }?;
        #[cfg(feature = "tracing")]
        tracing::trace!("Entry loaded.");
        let mut validation_layers_available = false;
        let mut debug_utils_available = false;

        let available_layers = unsafe { entry.enumerate_instance_layer_properties() }?;

        for layer in &available_layers {
            let layer_cstr = layer.layer_name_as_c_str().map_err(anyhow::Error::msg)?;
            let layer_name = layer_cstr.to_str().map_err(anyhow::Error::msg)?;

            if layer_name == VALIDATION_LAYER_NAME.to_str().map_err(anyhow::Error::msg)? {
                validation_layers_available = true;
                break;
            }
        }

        let mut available_extensions =
            unsafe { entry.enumerate_instance_extension_properties(None) }?;

        for ext in &available_extensions {
            if ext.extension_name_as_c_str().map_err(anyhow::Error::msg)? == DEBUG_UTILS_EXT_NAME {
                debug_utils_available = true;
            }
        }

        for layer in &available_layers {
            let layer_extensions = unsafe {
                entry.enumerate_instance_extension_properties(Some(
                    layer.layer_name_as_c_str().map_err(anyhow::Error::msg)?,
                ))
            }?;

            available_extensions.extend_from_slice(&layer_extensions);

            for ext in &layer_extensions {
                if ext.extension_name_as_c_str().map_err(anyhow::Error::msg)?
                    == DEBUG_UTILS_EXT_NAME
                {
                    debug_utils_available = true;
                }
            }
        }

        #[cfg(feature = "tracing")]
        tracing::trace!(validation_layers_available, debug_utils_available);

        let instance_api_version = unsafe { entry.try_enumerate_instance_version() }?.unwrap();

        Ok(Self {
            available_layers,
            available_extensions,
            debug_utils_available,
            validation_layers_available,
            instance_api_version,
            entry,
        })
    }

    pub fn is_extension_available(&self, extension: &CStr) -> crate::Result<bool> {
        for ext in &self.available_extensions {
            let cstr = ext.extension_name_as_c_str().map_err(anyhow::Error::msg)?;
            if cstr == extension {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn are_extensions_available<'a, I: IntoIterator<Item = &'a CStr>>(
        &self,
        extensions: I,
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

    pub fn is_layer_available(&self, layer: &CStr) -> crate::Result<bool> {
        for ext in &self.available_layers {
            let cstr = ext.layer_name_as_c_str().map_err(anyhow::Error::msg)?;
            if cstr == layer {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn are_layers_available<'a, I: IntoIterator<Item = &'a CStr>>(
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
