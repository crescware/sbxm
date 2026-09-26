//! `sync`の出力。

mod document;

pub use document::document;

#[cfg(test)]
#[path = "document_test.rs"]
mod document_test;
