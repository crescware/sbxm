use crate::command::{CommandOutcome, CommandSpec, HostEnvironment};
use crate::diagnostics::Result;

/// `docker version`のserver側versionを読み取る、read-onlyのprobe。
pub(super) fn version_probe(host: &dyn HostEnvironment) -> Result<CommandOutcome> {
    let spec = CommandSpec::probe("docker", &["version", "--format", "{{.Server.Version}}"]);
    host.run(&spec)
}
