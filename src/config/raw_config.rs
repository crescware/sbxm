use serde::{Deserialize, Serialize};

use super::RawFile;

/// YAMLの生表現。structへ変換する前にtop-level keyを検査する。
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct RawConfig {
    pub(super) version: Option<i64>,
    pub(super) language: Option<String>,
    // 未保存のidentityはkeyごと書かない。`null`を書くと、選ばれていないことと
    // 空を選んだことが同じ見た目になる。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) git_user_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) git_user_email: Option<String>,
    #[serde(default)]
    pub(super) files: Vec<RawFile>,
}
