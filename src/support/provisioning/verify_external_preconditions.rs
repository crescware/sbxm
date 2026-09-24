use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::repository::Provider;

use crate::support::{docker, secret};

use super::ExternalPreconditions;

/// custom secretの登録とDocker Engineへの疎通を、hostへ触れる前に確認する。
///
/// hostにあるrepositoryはhostから送るため、GitHub tokenの登録を求めない。
pub(crate) fn verify_external_preconditions(
    host: &dyn HostEnvironment,
    metadata: &ProjectMetadata,
) -> Result<ExternalPreconditions> {
    if metadata.repository.provider() == Provider::Github {
        secret::require_github(host, metadata.sandbox_name().as_str())?;
    }
    docker::require_reachable(host)?;
    Ok(ExternalPreconditions(()))
}
