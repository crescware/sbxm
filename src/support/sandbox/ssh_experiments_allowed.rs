use crate::boundary::host::protocol::SshFeature;
use crate::boundary::host::{
    CommandOutcome, CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass,
};
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

/// 管理者側のgateは観測するだけとし、SSHの修復で変更しない。
pub(super) fn ssh_experiments_allowed(host: &dyn HostEnvironment) -> Result<bool> {
    let spec = CommandSpec::probe(
        "sbx",
        &[
            "settings",
            "get",
            "platform.allowExperimentalFeatures",
            "--json",
        ],
    )
    .env(EnvPolicy::SbxSsh)
    .timeout(TimeoutClass::Probe);
    host.run(&spec)
        .and_then(CommandOutcome::require_success)
        .and_then(|outcome| SshFeature::parse_experimental(&outcome.stdout_text()))
        .map_err(|error| {
            let mut diagnostic = Diagnostic::new(
                ErrorId::RemoteSshSettingUnobservable,
                msg!("error-remote-ssh-setting-unobservable"),
            )
            .remediation(
                Remediation::text(msg!("remediation-remote-ssh-setting"))
                    .try_run("sbx settings get platform.allowExperimentalFeatures --json"),
            );
            for cause in error.diagnostics() {
                diagnostic = diagnostic.fact(Fact::cause(cause.id.as_str()));
                diagnostic.facts.extend(cause.facts.clone());
                if let Some(external) = &cause.external {
                    diagnostic = diagnostic.external(external.clone());
                }
            }
            Error::single(diagnostic)
        })
}
