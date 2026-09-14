use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;

use super::{SSH_ADD_NO_AGENT, exec, inner_exit_code, unobservable};

/// hostのSSH `AgentへSandboxの中から到達できるか`。
///
/// 露出とは、Sandbox内のprocessがagentへ接続して署名を求められることである。
/// 判定は`ssh-add -L`がagentへ接続できたかどうかで行う。`SSH_AUTH_SOCK`が
/// 指しているだけで誰も応えないsocketは、何にも署名できない。Docker Sandboxesは
/// 転送を無効にした後も、既存のSandboxへ`SSH_AUTH_SOCK`を渡し続けることがあり、
/// 変数だけを露出と見なすと、隔離できたSandboxを拒み続ける。
///
/// 露出していないことは、検査commandが答えた場合にだけ言える。検査が成立しなかった
/// 場合を「露出していない」へ丸めず、判定できないerrorとして返す。
pub fn ssh_agent_is_exposed(
    host: &dyn HostEnvironment,
    sandbox: &str,
) -> Result<Vec<&'static str>> {
    let socket = exec(host, sandbox, &["printenv", "SSH_AUTH_SOCK"])?;
    let socket_is_set = match inner_exit_code(&socket) {
        Some(0) if !socket.stdout_text().trim().is_empty() => true,
        // `printenv`は未設定のとき`1`で終わる。
        Some(0 | 1) => false,
        _ => return Err(unobservable(&socket, "SSH_AUTH_SOCK")),
    };

    let keys = exec(host, sandbox, &["ssh-add", "-L"])?;
    match inner_exit_code(&keys) {
        // 鍵の有無にかかわらず、agentへ接続できた時点で露出している。
        Some(0 | 1) => {}
        Some(SSH_ADD_NO_AGENT) => return Ok(Vec::new()),
        _ => return Err(unobservable(&keys, "ssh-add")),
    }

    let mut observed = Vec::new();
    if socket_is_set {
        observed.push("SSH_AUTH_SOCK is set");
    }
    observed.push("ssh-add reached an agent");
    Ok(observed)
}
