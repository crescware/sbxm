//! `registry.yaml`の生表現。読み取りと書き出しが同じ形を共有する。
//!
//! ここでは値の妥当性を判定しない。structへ写した後の検査は`parse`が持つ。
//!
//! 未知のkeyは受け付けない。観測から算出できる状態をregistryへ保存しないという
//! 不変条件は、書かれていたfieldを黙って無視しないことでしか守れない。

use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawRegistry {
    pub version: Option<i64>,
    #[serde(default)]
    pub projects: Vec<RawEntry>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RawEntry {
    pub canonical_id: Option<String>,
    pub project_root: Option<String>,
    pub provider: Option<String>,
    pub clone_transport: Option<String>,
    pub clone_url: Option<String>,
}
