//! directoryの検証と作成。
//!
//! symlinkと既存の非directoryは、内容を変更せず拒否する。permissionが過剰な既存
//! directoryも修復しない。

mod ensure_directory;
mod ensure_private_dir;
mod require_owned_directory;

pub use ensure_directory::ensure_directory;
pub use ensure_private_dir::ensure_private_dir;
pub use require_owned_directory::require_owned_directory;

#[cfg(test)]
#[path = "directory_test.rs"]
mod directory_test;
