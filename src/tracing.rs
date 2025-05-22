use std::borrow::Cow;
use std::ffi;
use ash::vk;
use ash::vk::DebugUtilsMessageSeverityFlagsEXT;

pub unsafe extern "system" fn vulkan_tracing_callback(
    message_severity: DebugUtilsMessageSeverityFlagsEXT,
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
        
        match message_severity {
            DebugUtilsMessageSeverityFlagsEXT::VERBOSE => {
                tracing::trace!("[{message_id_name} ({message_id_number})]: {message}");
            },
            DebugUtilsMessageSeverityFlagsEXT::INFO => {
                tracing::info!("[{message_id_name} ({message_id_number})]: {message}");
            },
            DebugUtilsMessageSeverityFlagsEXT::ERROR => {
                tracing::error!("[{message_id_name} ({message_id_number})]: {message}");
            },
            DebugUtilsMessageSeverityFlagsEXT::WARNING => {
                tracing::warn!("[{message_id_name} ({message_id_number})]: {message}");
            },
            _ => tracing::debug!("[{message_id_name} ({message_id_number})]: {message}")
        }

        vk::FALSE
    }
}