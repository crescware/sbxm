use crate::boundary::host::{CommandOutcome, CommandSpec, EnvPolicy, HostEnvironment};
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

/// SSH設定の観測と診断。中断は設定の障害へ変換せず、そのまま呼び出し元へ返す。
pub(super) fn ssh_setting<T>(
    host: &dyn HostEnvironment,
    key: &str,
    parse: impl FnOnce(&str) -> Result<T>,
) -> Result<T> {
    let spec =
        CommandSpec::probe("sbx", &["settings", "get", key, "--json"]).env(EnvPolicy::SbxSsh);
    host.run(&spec)
        .and_then(CommandOutcome::require_success)
        .and_then(|outcome| parse(&outcome.stdout_text()))
        .map_err(|error| {
            if error == Error::Canceled {
                return error;
            }
            let mut diagnostic = Diagnostic::new(
                ErrorId::RemoteSshSettingUnobservable,
                msg!("error-remote-ssh-setting-unobservable"),
            )
            .remediation(
                Remediation::text(msg!("remediation-remote-ssh-setting"))
                    .try_run(format!("sbx settings get {key} --json")),
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
