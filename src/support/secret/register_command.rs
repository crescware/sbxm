use super::GITHUB_SERVICE;

/// tokenを登録するcommand。
///
/// `add`の案内と、未登録で停止したときの是正指示で同じ文字列を使う。
///
/// 組み込み`github` serviceへ、このSandboxだけを対象に登録する。`--token`を付けずに
/// 実行すると値を対話で訊くため、tokenがshell historyに残らない。Sandbox限定の
/// service secretは、Sandboxが動作中でも登録した時点で効く。
pub fn register_command(sandbox: &str) -> String {
    format!("sbx secret set {GITHUB_SERVICE} --sandbox {sandbox}")
}
