use super::NotReusable;

/// 記録を使わずにversionを確認する理由。`--verbose`のtraceに残す。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Unused {
    /// 実行ファイルの識別情報を観測できなかった。
    ExecutableUnobserved,
    NotReusable(NotReusable),
    /// 記録fileが無い、またはこの道具の記録が無い。
    RecordMissing,
    /// 記録fileが要件を満たさない。
    RecordUnusable,
    /// 記録fileを読めない、または解釈できない。
    RecordCorrupt,
    /// 実行ファイルの識別情報が記録と違う。
    ExecutableChanged,
    /// 記録のdigestが内容と合わない。
    DigestMismatch,
    /// 記録されたversionが、このbuildの求める最低versionより古い。
    VersionBelowMinimum,
}

impl Unused {
    /// 翻訳しない安定した表記。
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Unused::ExecutableUnobserved => "executable-unobserved",
            Unused::NotReusable(reason) => reason.as_str(),
            Unused::RecordMissing => "record-missing",
            Unused::RecordUnusable => "record-unusable",
            Unused::RecordCorrupt => "record-corrupt",
            Unused::ExecutableChanged => "executable-changed",
            Unused::DigestMismatch => "digest-mismatch",
            Unused::VersionBelowMinimum => "recorded-version-below-minimum",
        }
    }
}
