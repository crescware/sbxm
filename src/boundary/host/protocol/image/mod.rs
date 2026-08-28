//! `docker image inspect`の解釈。

mod image_identity;
mod parse_image_inspect;

pub use image_identity::ImageIdentity;
pub use parse_image_inspect::parse_image_inspect;

#[cfg(test)]
#[path = "image_test.rs"]
mod image_test;
