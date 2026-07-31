//! `status`の出力。
//!
//! 表そのものが結論であるためsummaryを足さない。診断はstdoutの表へ列を増やさず、
//! stderrのdiagnosticとして出す。

mod global;
mod global_document;
mod project;
mod project_document;

pub use global::global;
pub use global_document::global_document;
pub use project::project;
pub use project_document::project_document;
