//! Host path導出と、filesystemに対する安全な基本操作。
//!
//! path構築には`PathBuf`を使い、symlinkを追跡しない。既存fileを暗黙に削除または
//! 上書きせず、config・metadataはatomic writeで置き換える。

mod atomic;
mod directory;
mod inspect;
mod lock;
mod project;
mod scope;

pub use atomic::{atomic_create, atomic_rename_into_place, atomic_replace};
pub use directory::{ensure_directory, ensure_private_dir};
pub use inspect::{
    display, is_symlink, lexically_standardize, permission_too_open, real_path, regular_file_exists,
};
pub use lock::{ExclusiveLock, acquire_exclusive_lock};
pub use project::{AbsoluteBasePath, PROJECT_DIR_SUFFIX, ProjectPaths};
pub use scope::PathScope;

use std::time::Duration;

/// `~/.sbxm`と案件の`.sbxm`のような、利用者専用directoryのpermission。
pub const PRIVATE_DIR_MODE: u32 = 0o700;
/// `~/.sbxm/config.toml`とproject metadataのpermission。
pub const PRIVATE_FILE_MODE: u32 = 0o600;
/// lock取得の待機上限。
pub const LOCK_TIMEOUT: Duration = Duration::from_secs(10);
