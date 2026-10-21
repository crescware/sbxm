use crate::boundary::host::protocol::CliVersion;
use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::design::ProgressSink;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use super::MINIMUM_HOST_GIT;

/// hostのgitが、このbuildの求める最低version以上であることを確かめる。
///
/// 古いgitは、途中の工程でgit自身のerrorとして失敗し、それまでの工程が済んだ状態を
/// 残しうる。そのため、hostのgitを使うcommandは最初にversionだけを読む。versionを
/// 読めなければ、対応するversionだと推測せずに止める。
pub fn require_supported_version(
    host: &dyn HostEnvironment,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    progress.step(msg!("progress-inspect-git-version"));
    if !host.command_exists("git") {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::HostCommandMissing,
                msg!("error-host-command-missing", program = "git"),
            )
            .remediation(msg!("remediation-install-command", program = "git")),
        ));
    }
    let spec = CommandSpec::capture("git", &["--version"])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::Probe);
    let output = host.run(&spec)?.require_success()?.stdout_text();
    let observed = CliVersion::extract_from_output(&output).ok_or_else(|| {
        Error::single(Diagnostic::new(
            ErrorId::HostGitVersionUnparseable,
            msg!(
                "error-host-git-version-unparseable",
                observed = output.trim()
            ),
        ))
    })?;
    if observed >= MINIMUM_HOST_GIT {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::HostGitTooOld,
            msg!(
                "error-host-git-too-old",
                observed = observed,
                minimum = MINIMUM_HOST_GIT
            ),
        )
        .remediation(msg!("remediation-host-git-too-old")),
    ))
}
