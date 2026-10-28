use super::ExecutableIdentity;

/// PATHから解決した実行ファイルの観測。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Executable {
    pub identity: ExecutableIdentity,
    /// 先頭が`#!`で始まるscriptか。scriptは別の実行ファイルを呼ぶwrapperでありうる。
    pub script: bool,
}
