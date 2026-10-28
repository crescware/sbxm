use super::NotReusable;

/// 確認が通ったのに記録しなかった理由。`--verbose`のtraceに残す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Unsaved {
    /// 実行ファイルの識別情報を観測できなかった。
    ExecutableUnobserved,
    NotReusable(NotReusable),
    /// `~/.sbxm`が無い。記録のためだけに作らない。
    StateDirectoryMissing,
    /// 記録fileが要件を満たさない。置き換えない。
    RecordUnusable,
    /// 描画または書き込みに失敗した。
    WriteFailed,
}

impl Unsaved {
    /// 翻訳しない安定した表記。
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Unsaved::ExecutableUnobserved => "executable-unobserved",
            Unsaved::NotReusable(reason) => reason.as_str(),
            Unsaved::StateDirectoryMissing => "state-directory-missing",
            Unsaved::RecordUnusable => "record-unusable",
            Unsaved::WriteFailed => "write-failed",
        }
    }
}
