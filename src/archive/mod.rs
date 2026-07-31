//! Image archiveの検証。
//!
//! `docker image save`が書いたarchiveが、buildして検証したimageそのものであることを、
//! Templateへloadする前に確かめる。archive全体を読まず、対応を判定できる最小限の
//! entryだけを取り出す。

mod archive_manifest;
mod block;
mod config_digest;
mod entry_name;
mod manifest_entry;
#[cfg(test)]
mod manifest_json;
mod max_entry_bytes;
mod octal;
mod read_entry;
mod read_manifest;
#[cfg(test)]
mod tar_bytes;
mod trimmed;
mod unusable;
mod verify_holds_image;

pub use archive_manifest::ArchiveManifest;
use block::BLOCK;
use config_digest::config_digest;
use entry_name::entry_name;
use manifest_entry::MANIFEST_ENTRY;
#[cfg(test)]
pub use manifest_json::manifest_json;
use max_entry_bytes::MAX_ENTRY_BYTES;
use octal::octal;
use read_entry::read_entry;
pub use read_manifest::read_manifest;
#[cfg(test)]
pub use tar_bytes::tar_bytes;
use trimmed::trimmed;
use unusable::unusable;
pub use verify_holds_image::verify_holds_image;

#[cfg(test)]
#[path = "archive_test.rs"]
mod archive_test;
