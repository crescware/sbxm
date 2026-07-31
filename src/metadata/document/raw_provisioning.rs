use serde::{Deserialize, Serialize};

use super::RawStartRef;

#[derive(Debug, Deserialize, Serialize)]
pub struct RawProvisioning {
    pub mode: Option<String>,
    /// 起点branchが未確定なら`null`。keyの欠落とは区別する。
    #[serde(default)]
    pub start_ref: RawStartRef,
    pub requested_worktrees: Option<i64>,
    pub dockerfile_sha256: Option<String>,
}
