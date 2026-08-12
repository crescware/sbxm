//! Sandbox内のroot filesystem使用量の観測。

mod disk_observation;
mod observe;

pub use disk_observation::DiskObservation;
pub use observe::observe;

#[cfg(test)]
#[path = "disk_test.rs"]
mod disk_test;
