//! `df -Pk /`の解釈。

mod parse_df;
mod root_disk_usage;

pub use parse_df::parse_df;
pub use root_disk_usage::RootDiskUsage;

#[cfg(test)]
#[path = "disk_test.rs"]
mod disk_test;
