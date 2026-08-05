use crate::command::{CommandOutcome, EnvPolicy, HostEnvironment, TerminalCommand, TimeoutClass};
use crate::design::ProgressSink;
use crate::diagnostics::Result;

use super::exec_arguments;

/// Sandbox内で、進捗をそのまま見せるcommandを実行する。
///
/// cloneやfetchのように、時間のかかる工程の進捗を実行中に見せるために使う。
pub fn exec_with_progress(
    host: &dyn HostEnvironment,
    sandbox: &str,
    args: &[&str],
    progress: &mut dyn ProgressSink,
) -> Result<CommandOutcome> {
    let full = exec_arguments(sandbox, None, args);
    let borrowed: Vec<&str> = full.iter().map(String::as_str).collect();
    let command = TerminalCommand::relayed("sbx", &borrowed)
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::RepositoryTransfer);
    host.run_with_terminal(&command, progress)
}
