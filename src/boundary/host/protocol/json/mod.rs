//! structured outputの共通部分。
//!
//! 解釈できない出力から状態を推測せず、同じ形のerrorで報告する。

mod string_field;

pub(super) use string_field::string_field;

#[cfg(test)]
#[path = "json_test.rs"]
mod json_test;
