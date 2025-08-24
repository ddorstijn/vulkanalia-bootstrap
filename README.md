# `Vulkanalia Bootstrap`&emsp; [![Latest Version]][crates.io] [![Rustc Version 1.36+]][rustc] ![license]

[Latest Version]: https://img.shields.io/crates/v/Vulkanalia_bootstrap.svg
[crates.io]: https://crates.io/crates/Vulkanalia_bootstrap
[Rustc Version 1.36+]: https://img.shields.io/badge/rustc-1.85+-lightgray.svg
[rustc]: https://blog.rust-lang.org/2025/02/20/Rust-1.85.0/
[license]: https://img.shields.io/badge/license-Apache--2.0_OR_MIT-blue

**Vulkanalia Bootstrap** is a utility library that jump starts initialization of Vulkan via the Vulkanalia bindings.

**Note:** This is a quick and dirty fork of ash bootrstrap. Feel free to use it as a quick starter. I will try to keep it somewhat up-to-date with ash-bootrstrap. Feel free to send improvements through a pull-request.

```toml
[dependencies]
vulkanalia_bootstrap = "0.1.3"
```

## Features

- Streamlined Vulkan initialization: Simplifies instance, device, and queue setup

- Window integration: Built-in support for surface creation via vulkanalia::window

- Tracing support: Optional integration with tracing crate

- Portability: macOS compatibility via portability feature

## Usage examples

```rust
fn main() -> anyhow::Result<()> {
    let instance = InstanceBuilder::new(None)
        .app_name("Example Vulkan Application")
        .engine_name("Example Vulkan Engine")
        .request_validation_layers(true)
        .use_default_tracing_messenger()
        .build()?;

    let physical_device = PhysicalDeviceSelector::new(instance.clone())
        .preferred_device_type(PreferredDeviceType::Discrete)
        .select()?;

    let device = Arc::new(DeviceBuilder::new(physical_device, instance.clone()).build()?);

    // You can get the inner handle that is used by vulkan
    // Or you can just pass it where the device handle is expected, because it implements AsRef.
    let _device_handle = device.handle();

    let (_graphics_queue_index, _graphics_queue) = device.get_queue(QueueType::Graphics)?;
    let swapchain_builder = SwapchainBuilder::new(instance.clone(), device.clone());

    let swapchain = swapchain_builder.build()?;

    // And right now we got rid of 400-500 lines of vulkan boilerplate just like that.
    // Now let's cleanup.

    swapchain.destroy();
    device.destroy();
    instance.destroy();
}
```

For more examples make sure to check `examples` directory.
