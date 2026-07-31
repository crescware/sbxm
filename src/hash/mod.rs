//! 決定的なcontent hash。
//!
//! Sandbox名とimage世代は同じhash関数から導出する。hexはlowercaseで統一する。

mod sha256_hex;
mod short_hex;
mod short_hex_length;

pub use sha256_hex::sha256_hex;
pub use short_hex::short_hex;
pub use short_hex_length::SHORT_HEX_LENGTH;

#[cfg(test)]
#[path = "hash_test.rs"]
mod hash_test;
