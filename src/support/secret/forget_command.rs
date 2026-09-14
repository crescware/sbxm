use super::GITHUB_SERVICE;

/// tokenの登録を解くcommand。
///
/// `--sandbox`でscopeを確定させる。scopeを渡さない実行はglobal scopeを対象にし、
/// ほかのSandboxが使う登録を消してしまう。
///
/// `--force`は`sbx`の確認promptを省く。消してよいかはsbxmが先に判定しており、
/// `destroy`は自前の確認も済ませている。
pub fn forget_command(sandbox: &str) -> String {
    format!("sbx secret rm {GITHUB_SERVICE} --sandbox {sandbox} --force")
}
