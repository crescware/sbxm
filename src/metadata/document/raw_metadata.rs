use serde::{Deserialize, Serialize};

use super::{RawGitIdentity, RawInitialProvisioning, RawProvisioning, RawRebuild, RawRepository};

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct RawMetadata {
    pub version: Option<i64>,
    pub repository: Option<RawRepository>,
    pub provisioning: Option<RawProvisioning>,
    pub git_identity: Option<RawGitIdentity>,
    /// 初回構築の最初のmutationから完成確認までだけ現れる。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub initial_provisioning: Option<RawInitialProvisioning>,
    /// Sandboxの切替中だけ現れる。切替中でなければkeyごと書かない。
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rebuild: Option<RawRebuild>,
}
