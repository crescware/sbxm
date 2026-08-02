//! 診断結果として表へ並べる値。
//!
//! 状態値は翻訳しない安定したenumとし、表示側は凡例で補う。

mod row;
mod status_value;

pub use row::Row;
pub use status_value::StatusValue;

#[cfg(test)]
#[path = "status_test.rs"]
mod status_test;
