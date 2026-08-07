use std::path::Path;

use crate::command::{CommandOutcome, HostEnvironment, TerminalCommand, TimeoutClass};
use crate::design::ProgressSink;
use crate::diagnostics::Result;
use crate::paths;

use super::diagnose_failure;

/// `docker image save`を実行する。進捗はdockerが出したまま転送する。
pub fn save(
    host: &dyn HostEnvironment,
    image: &str,
    output: &Path,
    progress: &mut dyn ProgressSink,
) -> Result<CommandOutcome> {
    let command = TerminalCommand::relayed(
        "docker",
        &["image", "save", image, "--output", &paths::display(output)],
    )
    .timeout(TimeoutClass::ImageBuild);
    host.run_with_terminal(&command, progress)
        .and_then(CommandOutcome::require_success)
        .map_err(|error| diagnose_failure(host, error))
}
