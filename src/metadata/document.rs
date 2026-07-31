//! `project.yaml`の生表現。読み取りと書き出しが同じ形を共有する。
//!
//! ここでは値の妥当性を判定しない。structへ写した後の検査は`parse`が持つ。

use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
    #[serde(default)]
    pub start_ref: RawStartRef,
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

/// 起点branchの記録の在り方。
///
/// keyが現れたことと、その値が`null`であることは別の事実である。`Option`を重ねると
/// どちらも`None`になり、「未確定として記録された」のか「記録が欠けている」のかを
/// 言い分けられない。3つの状態に名前を与えて、型がそのまま区別を担う。
#[derive(Debug, Default)]
pub(super) enum RawStartRef {
    /// keyそのものが無い。記録が欠けている。
    #[default]
    Missing,
    /// keyはあり、値は`null`。起点branchが未確定であると記録されている。
    Unset,
    /// keyがあり、branch名が記録されている。
    Named(String),
}

impl<'de> Deserialize<'de> for RawStartRef {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // ここへ来た時点でkeyは現れている。`Missing`はfieldの`default`だけが作る。
        Ok(Option::<String>::deserialize(deserializer)?.map_or(Self::Unset, Self::Named))
    }
}

impl Serialize for RawStartRef {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Self::Missing | Self::Unset => serializer.serialize_none(),
            Self::Named(value) => serializer.serialize_some(value),
        }
    }
}
