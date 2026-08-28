use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::project::SandboxName;

use crate::support::{docker, secret};

use super::ExternalPreconditions;

/// custom secretの登録とDocker Engineへの疎通を、hostへ触れる前に確認する。
pub(crate) fn verify_external_preconditions(
    host: &dyn HostEnvironment,
    name: &SandboxName,
) -> Result<ExternalPreconditions> {
    secret::require_github(host, name.as_str())?;
    docker::require_reachable(host)?;
    Ok(ExternalPreconditions(()))
}
