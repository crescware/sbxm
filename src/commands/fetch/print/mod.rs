//! `fetch`の出力。

mod auto_saved;
mod document;

pub use auto_saved::auto_saved;
pub use document::document;

#[cfg(test)]
#[path = "document_test.rs"]
mod document_test;
