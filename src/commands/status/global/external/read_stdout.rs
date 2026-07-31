use crate::command::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;

pub fn read_stdout(host: &dyn HostEnvironment, program: &str, args: &[&str]) -> Result<String> {
    let spec = CommandSpec::probe(program, args)
        .env(EnvPolicy::Inherit)
        .timeout(TimeoutClass::Probe);
    let outcome = host.run(&spec)?;
    let outcome = outcome.require_success()?;
    Ok(outcome.stdout_text())
}
