use crate::command::HostEnvironment;
use crate::diagnostics::Result;

use super::{SSH_ADD_NO_AGENT, exec, inner_exit_code, unobservable};

/// hostのSSH `AgentへSandboxの中から到達できるか`。
///
/// 露出していないことは、検査commandが答えた場合にだけ言える。検査が成立しなかった
/// 場合を「露出していない」へ丸めず、判定できないerrorとして返す。
pub fn ssh_agent_is_exposed(
    host: &dyn HostEnvironment,
    sandbox: &str,
) -> Result<Vec<&'static str>> {
    let mut observed = Vec::new();

    let socket = exec(host, sandbox, &["printenv", "SSH_AUTH_SOCK"])?;
    match inner_exit_code(&socket) {
        Some(0) if !socket.stdout_text().trim().is_empty() => observed.push("SSH_AUTH_SOCK is set"),
        // 値が空なら露出していない。`printenv`は未設定のとき`1`で終わる。
        Some(0 | 1) => {}
        _ => return Err(unobservable(&socket, "SSH_AUTH_SOCK")),
    }

    let keys = exec(host, sandbox, &["ssh-add", "-L"])?;
    match inner_exit_code(&keys) {
        // 鍵の有無にかかわらず、agentへ接続できた時点で露出している。
        Some(0 | 1) => observed.push("ssh-add reached an agent"),
        Some(SSH_ADD_NO_AGENT) => {}
        _ => return Err(unobservable(&keys, "ssh-add")),
    }

    Ok(observed)
}
