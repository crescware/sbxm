//! Image archiveの検証。
//!
//! `docker image save`が書いたarchiveが、buildして検証したimageそのものであることを、
//! Templateへloadする前に確かめる。archive全体を読まず、対応を判定できる最小限の
//! entryだけを取り出す。

mod archive_manifest;
mod block;
mod config_digest;
mod entry_name;
#[cfg(test)]
mod fake;
mod manifest_entry;
mod max_entry_bytes;
mod octal;
mod read_entry;
mod read_manifest;
mod reported;
mod trimmed;
mod unreadable;
mod unusable;
mod verify_holds_image;

pub use archive_manifest::ArchiveManifest;
use block::BLOCK;
use config_digest::config_digest;
use entry_name::entry_name;
#[cfg(test)]
pub use fake::{manifest_json, tar_bytes};
use manifest_entry::MANIFEST_ENTRY;
use max_entry_bytes::MAX_ENTRY_BYTES;
use octal::octal;
use read_entry::read_entry;
pub use read_manifest::read_manifest;
use reported::reported;
use trimmed::trimmed;
use unreadable::unreadable;
use unusable::unusable;
pub use verify_holds_image::verify_holds_image;

#[cfg(test)]
#[path = "archive_test.rs"]
mod archive_test;
