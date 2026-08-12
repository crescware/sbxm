use crate::command::{HostEnvironment, TerminalCommand};
use crate::design::ExternalOutput;
use crate::diagnostics::Result;

use super::Prepared;

/// terminalをSSHへ引き渡す。
///
/// `SSHのexit` statusが0なら成功とし、非ゼロは理由を推測せず外部command失敗とする。
/// `Prepared`を所有することで、SSHの直接の子processが終了した直後にsession leaseを
/// 解放し、呼び出し側が結果を表示するときまで保持しない。
pub fn connect(
    host: &dyn HostEnvironment,
    prepared: Prepared,
    output: &mut dyn ExternalOutput,
) -> Result<()> {
    let remote_command = format!(
        "cd {} && exec \"${{SHELL:-/bin/sh}}\" -l",
        shell_quote(&prepared.working_directory)
    );
    let args = ["-t", prepared.ssh_host.as_str(), remote_command.as_str()];
    let command = TerminalCommand::handed_over("ssh", &args);
    host.run_with_terminal(&command, output)?
        .require_success()?;
    Ok(())
}

/// Sandbox内で評価するcommandへpathを安全に埋め込む。
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
