use std::fmt::{Display, Formatter};
use vulkanalia::vk;

pub struct Version(u32);

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}.{}",
            vk::version_major(self.0),
            vk::version_minor(self.0),
            vk::version_patch(self.0)
        )
    }
}

impl Version {
    pub fn new(version: u32) -> Self {
        Self(version)
    }
}
