use super::FakeSbx;

/// host側のSSH `Agentへ到達できないSandbox`。
pub fn isolated_agent(host: FakeSbx, sandbox: &str) -> FakeSbx {
    host.answering(&format!("exec {sandbox} -- printenv SSH_AUTH_SOCK"), 1, "")
        .answering(&format!("exec {sandbox} -- ssh-add -L"), 2, "")
}
