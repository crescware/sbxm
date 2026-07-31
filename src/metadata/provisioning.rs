use super::CreationMode;

/// 利用者が要求した目標構成。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provisioning {
    pub mode: CreationMode,
    /// 起点branch。attached modeではremote default branchを解決するまで未確定を許す。
    pub start_ref: Option<String>,
    pub requested_worktrees: u32,
    /// 初回構築中は採用世代、構築完了後は適用済みのDockerfile hash。
    pub dockerfile_sha256: String,
}
