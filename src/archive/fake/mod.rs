//! archiveのtestが組み立てる`docker image save`の出力。

mod manifest_json;
mod tar_bytes;

pub use manifest_json::manifest_json;
pub use tar_bytes::tar_bytes;
