//! Host path導出と、filesystemに対する安全な基本操作。
//!
//! path構築には`PathBuf`を使い、symlinkを追跡しない。既存fileを暗黙に削除または
//! 上書きせず、config・metadataはatomic writeで置き換える。

mod atomic;
mod directory;
mod inspect;
mod lock;
mod lock_timeout;
mod private_dir_mode;
mod private_file_mode;
mod project;
mod scope;

pub use atomic::{atomic_create, atomic_rename_into_place, atomic_replace};
pub use directory::{ensure_directory, ensure_private_dir, require_owned_directory};
pub use inspect::{
    directory_exists, display, is_symlink, lexically_standardize, permission_too_open, real_path,
    regular_file_exists,
};
pub use lock::{ExclusiveLock, acquire_exclusive_lock};
pub use lock_timeout::LOCK_TIMEOUT;
pub use private_dir_mode::PRIVATE_DIR_MODE;
pub use private_file_mode::PRIVATE_FILE_MODE;
pub use project::{ProjectParent, ProjectPaths};
pub use scope::PathScope;
