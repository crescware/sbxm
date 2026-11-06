//! Sandbox内のroot filesystem使用量の観測。

mod attach_on_failure;
mod disk_meter;
mod disk_observation;
mod format_capacity;
mod observe;
mod storage;

pub use attach_on_failure::attach_on_failure;
pub use disk_meter::disk_meter;
pub use disk_observation::DiskObservation;
pub use format_capacity::format_capacity;
pub use observe::observe;
pub use storage::Storage;

#[cfg(test)]
#[path = "disk_test.rs"]
mod disk_test;

#[cfg(test)]
#[path = "disk_meter_test.rs"]
mod disk_meter_test;

#[cfg(test)]
#[path = "attach_on_failure_test.rs"]
mod attach_on_failure_test;
