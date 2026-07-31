/// `rebuild`のSandbox切替中だけ存在する適用予定世代。
///
/// `rebuild`が新世代の成果物を揃えた時点で記録し、切替完了で消す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebuildIntent {
    pub target_dockerfile_sha256: String,
    pub previous_dockerfile_sha256: String,
}
