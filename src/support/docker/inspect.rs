use crate::command::{CommandOutcome, CommandSpec, HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;

/// `docker image inspect`を実行する。
pub fn inspect(host: &dyn HostEnvironment, name: &str) -> Result<CommandOutcome> {
    let spec = CommandSpec::capture("docker", &["image", "inspect", name])
        .timeout(TimeoutClass::LocalFilesystem);
    host.run(&spec)
}
