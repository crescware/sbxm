//! Sandbox内のroot filesystem使用量の観測。

mod attach_on_failure;
mod disk_observation;
mod format_gib;
mod format_percent;
mod observe;

pub use attach_on_failure::attach_on_failure;
pub use disk_observation::DiskObservation;
pub use format_gib::format_gib;
pub use format_percent::format_percent;
pub use observe::observe;

#[cfg(test)]
#[path = "disk_test.rs"]
mod disk_test;

#[cfg(test)]
#[path = "attach_on_failure_test.rs"]
mod attach_on_failure_test;
