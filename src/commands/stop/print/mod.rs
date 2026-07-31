//! `stop`の出力。
//!
//! 対象ごとの結果の表そのものが結論であるため、summaryを足さない。失敗した対象の診断は
//! 表へ列を増やさず、stderrの別blockとして出す。

mod document;
mod report;

pub use document::document;
pub use report::report;
