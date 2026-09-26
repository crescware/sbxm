use crate::support::sandbox::ssh_host;

/// hostから`sandbox`の`git_dir`へ届くgit URL。
///
/// `sbxm open`と同じ宛先へsshする。通信はhostから始まり、Sandboxへhostへの経路を
/// 与えない。
pub fn sandbox_remote(sandbox: &str, git_dir: &str) -> String {
    format!("ssh://{}{git_dir}", ssh_host(sandbox))
}
