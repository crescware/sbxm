/// 外部commandの失敗を、翻訳せず原文のまま持つ。
///
/// stderrはFTL placeholderへ埋め込まず、localized説明とは別blockで表示する。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalFailure {
    pub program: String,
    /// secret値を含まないことが保証されたargumentだけを保持する。
    pub safe_args: Vec<String>,
    /// 実行時の作業directory。指定していない場合は`None`。
    pub working_dir: Option<std::path::PathBuf>,
    /// 外部commandのexit statusを原値のまま示す文字列。
    pub exit_status: String,
    pub stderr: Vec<u8>,
    /// stderrをUTF-8として解釈する際にlossy変換が発生したか。
    pub stderr_lossy: bool,
}

#[cfg(test)]
#[path = "external_failure_test.rs"]
mod external_failure_test;
