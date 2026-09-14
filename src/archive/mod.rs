//! Image archiveの検証。
//!
//! `docker image save`が書いたarchiveが、buildして検証したimageそのものであることを、
//! Templateへloadする前に確かめる。archive全体を読まず、対応を判定できる最小限の
//! entryだけを取り出す。

mod archive_manifest;
mod block;
mod entry_name;
#[cfg(test)]
mod fake;
mod index_entry;
mod manifest_entry;
mod max_entry_bytes;
mod normalized_digest;
mod octal;
mod read_entry;
mod read_image_ids;
mod read_manifest;
mod reported;
mod trimmed;
mod unreadable;
mod unusable;
mod verify_holds_image;

pub use archive_manifest::ArchiveManifest;
use block::BLOCK;
use entry_name::entry_name;
#[cfg(test)]
pub use fake::{index_json, manifest_json, tar_bytes};
use index_entry::INDEX_ENTRY;
use manifest_entry::MANIFEST_ENTRY;
use max_entry_bytes::MAX_ENTRY_BYTES;
use normalized_digest::normalized_digest;
use octal::octal;
use read_entry::read_entry;
pub use read_image_ids::read_image_ids;
pub use read_manifest::read_manifest;
use reported::reported;
use trimmed::trimmed;
use unreadable::unreadable;
use unusable::unusable;
pub use verify_holds_image::verify_holds_image;

#[cfg(test)]
#[path = "archive_test.rs"]
mod archive_test;
