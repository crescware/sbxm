use std::path::Path;

use crate::diagnostics::Result;
use crate::paths::{self, PathScope};
use crate::project::SandboxName;

use super::workspace_path;

/// 中立workspace directoryがhostに在るかを実測する。
///
/// runtimeのrecordが在ることは、そのrecordがmount元として宣言するdirectoryが在ること
/// を含まない。一覧が示すpath文字列を実在の根拠に読み替えず、hostを直接見る。
///
/// 不在は`false`で答え、観測できなかった場合は`false`にせず理由を返す。実在を問う
/// 場所はこの1箇所とし、判定の仕方が呼び出し側ごとに分かれないようにする。
pub fn workspace_exists(workspace_root: &Path, sandbox: &SandboxName) -> Result<bool> {
    paths::directory_exists(
        &workspace_path(workspace_root, sandbox),
        PathScope::ProjectPath,
    )
}
