use crate::boundary::host::protocol::SshFeature;
use crate::boundary::host::{
    CommandOutcome, CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass,
};
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

/// daemonが動いている場合だけ呼ぶ。設定の読み取り自体がdaemonを起動し得る。
pub fn ssh_feature(host: &dyn HostEnvironment) -> Result<SshFeature> {
    let spec = CommandSpec::probe("sbx", &["settings", "get", "feature.ssh", "--json"])
        .env(EnvPolicy::SbxSsh)
        .timeout(TimeoutClass::Probe);
    let result = host
        .run(&spec)
        .and_then(CommandOutcome::require_success)
        .and_then(|outcome| SshFeature::parse(&outcome.stdout_text()));
    result.map_err(|error| {
        let mut diagnostic = Diagnostic::new(
            ErrorId::RemoteSshSettingUnobservable,
            msg!("error-remote-ssh-setting-unobservable"),
        )
        .remediation(
            Remediation::text(msg!("remediation-remote-ssh-setting"))
                .try_run("sbx settings get feature.ssh --json"),
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
