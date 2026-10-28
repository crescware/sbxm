use crate::boundary::host::ExecutableIdentity;
use crate::boundary::host::protocol::CliVersion;

use super::digest;

/// 最低versionを満たすと確認できた実行ファイルの記録。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RecordedTool {
    pub(super) identity: ExecutableIdentity,
    /// 確認時に読んだversion。
    pub(super) observed: CliVersion,
    /// 記録の内容から計算したdigest。読むときに再計算し、一致しなければ記録を使わない。
    pub(super) digest: String,
}

impl RecordedTool {
    pub(super) fn new(
        program: &str,
        identity: ExecutableIdentity,
        observed: CliVersion,
    ) -> RecordedTool {
        let digest = digest(program, &identity, observed);
        RecordedTool {
            identity,
            observed,
            digest,
        }
    }
}
