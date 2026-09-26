/// hostのgitが、Sandboxのrepositoryへ届くために起動するssh。
///
/// promptを待たずに失敗させる。hostのSSH agentもport forwardingもSandboxへ渡さない。
/// 利用者のssh設定にこれらを許す項目があっても、command lineの指定が先に効く。
const SANDBOX_SSH_COMMAND: &str =
    "ssh -o BatchMode=yes -o ForwardAgent=no -o ClearAllForwardings=yes";

/// `sandbox_remote`へ届くgitに付ける設定。
///
/// `GIT_SSH_COMMAND`などの環境変数は、hostのrepositoryで走らせるgitから取り除いてある。
pub fn sandbox_ssh_config() -> String {
    format!("core.sshCommand={SANDBOX_SSH_COMMAND}")
}
