use std::sync::Arc;
use ash_bootstrap::{InstanceBuilder, PhysicalDeviceSelector};

fn main() {
    let instance = Arc::new(
        InstanceBuilder::new(None).build().unwrap()
    );

    let physical_device = PhysicalDeviceSelector::new(instance.clone())
        .select() // SIGSEVs here
        .unwrap();
}