//! `apply`の出力。
//!
//! worktreeとfileはそれぞれ独立した結果であるため、適用した範囲だけをsummaryにする。

mod all_document;
mod all_report;
mod document;

pub use all_document::all_document;
pub use all_report::all_report;
pub use document::document;
