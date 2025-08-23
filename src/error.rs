use thiserror::Error;
use vulkanalia::vk;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Instance error: {0}")]
    Instance(#[from] InstanceError),
    #[error("Physical device error: {0}")]
    PhysicalDevice(#[from] PhysicalDeviceError),
    #[error("Queue error: {0}")]
    Queue(#[from] QueueError),
    #[error("Swapchain error: {0}")]
    Swapchain(#[from] SwapchainError),
    #[error("Vulkanalia loading error: {0}")]
    AshLoading(#[from] libloading::Error),
    #[error("Vulkan error: {0}")]
    Vulkan(#[from] vulkanalia::vk::Result),
    #[error("Vulkan error: {0}")]
    VulkanErr(#[from] vk::ErrorCode),
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Debug, PartialOrd, PartialEq, Eq, Ord, Error)]
pub enum InstanceError {
    #[error("Vulkan unavailable")]
    VulkanUnavailable,
    #[error("Vulkan version {0} unavailable")]
    VulkanVersionUnavailable(String),
    #[error("Vulkan 1.1 unavailable")]
    VulkanVersion11Unavailable,
    #[error("Vulkan 1.2 unavailable")]
    VulkanVersion12Unavailable,
    #[error("Vulkan 1.3 unavailable")]
    VulkanVersion13Unavailable,
    #[error("Vulkan 1.4 unavailable")]
    VulkanVersion14Unavailable,
    #[error("Failed to create instance")]
    FailedCreateInstance,
    #[error("Failed to create debug messenger")]
    FailedCreateDebugMessenger,
    #[error("Failed to find requested layers: {0:#?}")]
    RequestedLayersNotPresent(Vec<vk::ExtensionName>),
    #[error("Failed to find requested extensions: {0:#?}")]
    RequestedExtensionsNotPresent(Vec<vk::ExtensionName>),
    #[error("Failed to find windowing extensions: {0:#?}")]
    WindowingExtensionsNotPresent(Vec<vk::ExtensionName>),
}

#[derive(Debug, PartialOrd, PartialEq, Eq, Ord, Error)]
pub enum PhysicalDeviceError {
    #[error("No surface provided")]
    NoSurfaceProvided,
    #[error("Failed to enumerate physical devices")]
    FailedToEnumeratePhysicalDevices,
    #[error("No physical devices found")]
    NoPhysicalDevicesFound,
    #[error("No suitable device")]
    NoSuitableDevice,
}

#[derive(Debug, PartialOrd, PartialEq, Eq, Ord, Error)]
pub enum QueueError {
    #[error("Present unavailable")]
    PresentUnavailable,
    #[error("Graphics unavailable")]
    GraphicsUnavailable,
    #[error("Compute unavailable")]
    ComputeUnavailable,
    #[error("Transfer unavailable")]
    TransferUnavailable,
    #[error("Queue index out of bounds")]
    QueueIndexOutOfBounds,
    #[error("Invalid queue family index")]
    InvalidQueueFamilyIndex,
}

#[derive(Debug, PartialEq, Eq)]
pub struct FormatError {
    pub available: Vec<vk::SurfaceFormatKHR>,
    pub desired: Vec<vk::SurfaceFormatKHR>,
}

#[derive(Debug, PartialEq, Eq, Error)]
pub enum SwapchainError {
    #[error("Surface handle not provided")]
    SurfaceHandleNotProvided,
    #[error("Failed query surface support details")]
    FailedQuerySurfaceSupportDetails,
    #[error("Failed to create swapchain")]
    FailedCreateSwapchain,
    #[error("Failed to get swapchain images")]
    FailedGetSwapchainImages,
    #[error("Failed to create swapchain image views")]
    FailedCreateSwapchainImageViews,
    #[error("Required min image count too low")]
    RequiredMinImageCountTooLow,
    #[error("Required usage not supported")]
    RequiredUsageNotSupported,
    #[error("No suitable desired format")]
    NoSuitableDesiredFormat(FormatError),
}

pub type Result<T> = std::result::Result<T, Error>;
