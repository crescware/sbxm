//! structured outputの共通部分。
//!
//! 解釈できない出力から状態を推測せず、同じ形のerrorで報告する。

mod json_documents;
mod string_field;
mod wrapped_documents;

pub(super) use json_documents::json_documents;
pub(super) use string_field::string_field;
pub(super) use wrapped_documents::wrapped_documents;
