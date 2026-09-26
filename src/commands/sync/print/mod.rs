//! `sync`の出力。

mod document;
mod report;

pub use document::document;
pub use report::report;

#[cfg(test)]
#[path = "document_test.rs"]
mod document_test;
