use crate::boundary::host::HostEnvironment;
use crate::design::ProgressSink;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::msg;

use crate::support::{docker, host_sync, sandbox, secret};

use super::ExternalPreconditions;

/// custom secretの登録とDocker Engineへの疎通を、hostへ触れる前に確認する。
///
/// hostにあるrepositoryはhostから送るため、GitHub tokenの登録を求めない。代わりに、
/// そのrepositoryが登録した場所にあり、送るものを持つことと、送るためのsshでSandboxへ
/// つながる設定があることを確かめる。imageやSandboxを作ってから送れないと分かるのでは
/// 遅い。
pub(crate) fn verify_external_preconditions(
    host: &dyn HostEnvironment,
    metadata: &ProjectMetadata,
    progress: &mut dyn ProgressSink,
) -> Result<ExternalPreconditions> {
    if metadata.repository.uses_github_token() {
        progress.step(msg!("progress-inspect-credentials"));
        secret::require_github(host, metadata.sandbox_name().as_str())?;
    }
    if let Some(repository) = metadata.repository.host_path() {
        progress.step(msg!("progress-inspect-host-repository"));
        host_sync::require_something_to_send(host, repository)?;
        progress.step(msg!("progress-inspect-remote-ssh"));
        sandbox::require_ssh(host, metadata.sandbox_name().as_str())?;
    }
    progress.step(msg!("progress-inspect-docker"));
    docker::require_reachable(host)?;
    Ok(ExternalPreconditions(()))
}
