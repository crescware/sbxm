//! `docker image save`が書くarchiveの偽物。

mod image_archive_bytes;
mod index_id;

pub use image_archive_bytes::image_archive_bytes;
pub use index_id::index_id;
