use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::repository::Provider;

use crate::support::{bundle, docker, secret};

use super::ExternalPreconditions;

/// custom secretの登録とDocker Engineへの疎通を、hostへ触れる前に確認する。
///
/// hostにあるrepositoryはhostから送るため、GitHub tokenの登録を求めない。代わりに、
/// そのrepositoryが登録した場所にあり、送るものを持つことを確かめる。imageやSandboxを
/// 作ってから送れないと分かるのでは遅い。
pub(crate) fn verify_external_preconditions(
    host: &dyn HostEnvironment,
    metadata: &ProjectMetadata,
) -> Result<ExternalPreconditions> {
    match metadata.repository.provider() {
        Provider::Github => {
            secret::require_github(host, metadata.sandbox_name().as_str())?;
        }
        Provider::Local => {
            let repository = Path::new(metadata.repository.clone_url());
            bundle::require_something_to_send(host, repository)?;
        }
    }
    docker::require_reachable(host)?;
    Ok(ExternalPreconditions(()))
}
