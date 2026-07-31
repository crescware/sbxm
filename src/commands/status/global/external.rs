//! 外部commandの実行結果の読み取り。

use crate::command::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::error::{Error, Result};

pub(super) fn read_stdout(
    host: &dyn HostEnvironment,
    program: &str,
    args: &[&str],
) -> Result<String> {
    let spec = CommandSpec::probe(program, args)
        .env(EnvPolicy::Inherit)
        .timeout(TimeoutClass::Probe);
    let outcome = host.run(&spec)?;
    let outcome = outcome.require_success()?;
    Ok(outcome.stdout_text())
}

pub(super) fn describe(error: &Error) -> String {
    error.diagnostics().first().map_or_else(
        || "canceled".to_string(),
        |diagnostic| diagnostic.id.to_string(),
    )
}

pub(super) fn external_of(error: &Error) -> Option<crate::error::ExternalFailure> {
    error
        .diagnostics()
        .first()
        .and_then(|diagnostic| diagnostic.external.clone())
}
