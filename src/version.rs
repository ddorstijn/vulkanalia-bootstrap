use ash::vk;
use std::fmt::{Display, Formatter};

pub struct Version(u32);

impl Display for Version {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}.{}",
            vk::api_version_major(self.0),
            vk::api_version_minor(self.0),
            vk::api_version_patch(self.0)
        )
    }
}

impl Version {
    pub fn new(version: u32) -> Self {
        Self(version)
    }
}
