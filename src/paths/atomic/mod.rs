//! atomic write。
//!
//! 既存fileを暗黙に削除または上書きせず、config・metadataは検証を通してから
//! 置き換える。

mod atomic_create;
mod atomic_rename_into_place;
mod atomic_replace;
mod atomic_write_with_precondition;
mod replaceable_identity;
mod temp_path_for;

pub use atomic_create::atomic_create;
pub use atomic_rename_into_place::atomic_rename_into_place;
pub use atomic_replace::atomic_replace;
use atomic_write_with_precondition::atomic_write_with_precondition;
use replaceable_identity::replaceable_identity;
use temp_path_for::temp_path_for;

#[cfg(test)]
#[path = "atomic_test.rs"]
mod atomic_test;
