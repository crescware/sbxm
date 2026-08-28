use crate::boundary::host::{
    CommandOutcome, CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass,
};
use crate::diagnostics::Result;

use super::exec_arguments;

pub(super) fn run_exec(
    host: &dyn HostEnvironment,
    sandbox: &str,
    user: Option<&str>,
    args: &[&str],
    timeout: TimeoutClass,
) -> Result<CommandOutcome> {
    let full = exec_arguments(sandbox, user, args);
    let borrowed: Vec<&str> = full.iter().map(String::as_str).collect();
    let spec = CommandSpec::capture("sbx", &borrowed)
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(timeout);
    host.run(&spec)
}
