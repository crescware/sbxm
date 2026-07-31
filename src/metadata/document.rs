//! `project.yaml`の生表現。読み取りと書き出しが同じ形を共有する。
//!
//! ここでは値の妥当性を判定しない。structへ写した後の検査は`parse`が持つ。

use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Default, Deserialize, Serialize)]
pub(super) struct RawMetadata {
    pub version: Option<i64>,
    pub repository: Option<RawRepository>,
    pub provisioning: Option<RawProvisioning>,
    pub git_identity: Option<RawGitIdentity>,
    /// Sandboxの切替中だけ現れる。切替中でなければkeyごと書かない。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rebuild: Option<RawRebuild>,
}

/// 登録対象の不変なrepository identity。
///
/// clone URLからtransportを実行時に推測し直さないよう、解釈済みの値をそのまま持つ。
#[derive(Debug, Deserialize, Serialize)]
pub(super) struct RawRepository {
    pub provider: Option<String>,
    pub owner: Option<String>,
    pub name: Option<String>,
    pub canonical_id: Option<String>,
    pub clone_transport: Option<String>,
    pub clone_url: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct RawProvisioning {
    pub mode: Option<String>,
    /// 起点branchが未確定なら`null`。keyの欠落とは区別する。
    #[serde(default, deserialize_with = "present_option")]
    pub start_ref: Option<Option<String>>,
    pub requested_worktrees: Option<i64>,
    pub dockerfile_sha256: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct RawGitIdentity {
    pub user_name: Option<String>,
    pub user_email: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub(super) struct RawRebuild {
    pub target_dockerfile_sha256: Option<String>,
    pub previous_dockerfile_sha256: Option<String>,
}

/// keyが現れたことと、その値が`null`であることを区別して読む。
///
/// `Option<Option<String>>`をそのまま読むと`null`もkeyの欠落もどちらも`None`になり、
/// 「未確定として記録された」のか「記録が欠けている」のかを言い分けられない。
fn present_option<'de, D>(deserializer: D) -> Result<Option<Option<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<String>::deserialize(deserializer).map(Some)
}
