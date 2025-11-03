//! Vulkanalia-bootstrap is a rust implementation of vulkan-bootstrap for Rust for use with the vulkanalia library.
//! It is a fork of the ash-bootstrap library with adjusted imports and function calls.
//!
//! # Quick start
//!
//! The bootstrap library's goal is to make the tedious setup of Vulkan a little bit more bearable.
//! This is done by providing a bunch of builder that provide help by implementing the boilerplate for you.
//! From instance to swapchain creation, hunderds of lines of code are implemented for you already.
//! It tries to not be in the way as much as possible in the rest of your Vulkan application.   
//!
//! ``` no_run
//! fn main() -> anyhow::Result<()> {
//!    let instance = InstanceBuilder::new(None)
//!        .app_name("Example Vulkan Application")
//!        .engine_name("Example Vulkan Engine")
//!        .request_validation_layers(true)
//!        .use_default_tracing_messenger()
//!        .build()?;
//!
//!    let physical_device = PhysicalDeviceSelector::new(instance.clone())
//!        .preferred_device_type(PreferredDeviceType::Discrete)
//!        .select()?;
//!
//!    let device = Arc::new(DeviceBuilder::new(physical_device, instance.clone()).build()?);
//!
//!    // You can get the inner handle that is used by vulkan
//!    // Or you can just pass it where the device handle is expected, because it implements AsRef.
//!    let _device_handle = device.handle();
//!
//!    let (_graphics_queue_index, _graphics_queue) = device.get_queue(QueueType::Graphics)?;
//!    let swapchain_builder = SwapchainBuilder::new(instance.clone(), device.clone());
//!
//!    let swapchain = swapchain_builder.build()?;
//!
//!    // And right now we got rid of 400-500 lines of vulkan boilerplate just like that.
//!    // Now let's cleanup.
//!
//!    swapchain.destroy();
//!    device.destroy();
//!    instance.destroy();
//!}
//! ```

mod device;
mod error;
mod instance;
mod swapchain;
mod system_info;
#[cfg(feature = "enable_tracing")]
mod tracing;

pub use device::{
    Device, DeviceBuilder, PhysicalDevice, PhysicalDeviceSelector, PreferredDeviceType, QueueType,
};
pub use error::*;
pub use instance::{Instance, InstanceBuilder};
pub use swapchain::{Swapchain, SwapchainBuilder};
