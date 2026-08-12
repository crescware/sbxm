//! pathの観測。
//!
//! symlinkを追跡せず、所有者とpermissionは観測した値で判定する。ここにあるものは
//! filesystemを変更しない。

mod current_user;
mod directory_exists;
mod display;
mod file_identity;
mod format_mode;
mod is_symlink;
mod lexically_standardize;
mod permission_too_open;
mod real_path;
mod regular_file_exists;
mod require_owned_by_current_user;
mod require_private_file;
mod unexpected_type;

pub(super) use current_user::current_user;
pub use directory_exists::directory_exists;
pub use display::display;
pub use file_identity::FileIdentity;
pub use format_mode::format_mode;
pub use is_symlink::is_symlink;
pub use lexically_standardize::lexically_standardize;
pub use permission_too_open::permission_too_open;
pub use real_path::real_path;
pub use regular_file_exists::regular_file_exists;
pub(super) use require_owned_by_current_user::require_owned_by_current_user;
pub(super) use require_private_file::require_private_file;
pub(super) use unexpected_type::unexpected_type;

#[cfg(test)]
#[path = "inspect_test.rs"]
mod inspect_test;
