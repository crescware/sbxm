use super::InitialProvisioningFile;

/// 初回構築が始まったことと、復旧先のgenerationを固定する記録。
///
/// これは進捗cacheではない。host上の可変成果物がどこまでできたかは毎回観測し、
/// この記録にあるgenerationと入力snapshotだけを復旧先の正本として使う。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InitialProvisioningIntent {
    pub target_dockerfile_sha256: String,
    pub files: Vec<InitialProvisioningFile>,
}
