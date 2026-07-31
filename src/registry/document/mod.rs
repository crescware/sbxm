//! `registry.yaml`の生表現。読み取りと書き出しが同じ形を共有する。
//!
//! ここでは値の妥当性を判定しない。structへ写した後の検査は`parse`が持つ。
//!
//! 未知のkeyは受け付けない。観測から算出できる状態をregistryへ保存しないという
//! 不変条件は、書かれていたfieldを黙って無視しないことでしか守れない。

mod raw_entry;
mod raw_registry;

pub(super) use raw_entry::RawEntry;
pub(super) use raw_registry::RawRegistry;
