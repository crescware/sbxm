use crate::command::{CommandOutcome, HostEnvironment, TerminalCommand, TimeoutClass};
use crate::design::ProgressSink;
use crate::diagnostics::Result;

/// `docker build`を実行する。進捗はdockerが出したまま転送する。
pub fn build(
    host: &dyn HostEnvironment,
    args: &[&str],
    progress: &mut dyn ProgressSink,
) -> Result<CommandOutcome> {
    let command = TerminalCommand::relayed("docker", &[&["build"], args].concat())
        .timeout(TimeoutClass::ImageBuild);
    host.run_with_terminal(&command, progress)
}
