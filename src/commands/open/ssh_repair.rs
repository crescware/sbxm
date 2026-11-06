use std::time::Duration;

use crate::boundary::host::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::msg;
use crate::support::sandbox;
use crate::time::Clock;

/// 観測した失敗に対応する、利用者が承認する1操作。
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum SshRepair {
    Setup,
    Enable,
    Restart,
}

impl SshRepair {
    pub fn for_error(error: &Error) -> Option<Self> {
        if error.contains_id(ErrorId::RemoteSshUnconfigured) {
            Some(Self::Setup)
        } else if error.contains_id(ErrorId::RemoteSshDisabled) {
            Some(Self::Enable)
        } else if error.contains_id(ErrorId::RemoteSshUnavailable)
            || error.contains_id(ErrorId::RemoteSshRestartRequired)
        {
            Some(Self::Restart)
        } else {
            None
        }
    }

    pub fn description(self) -> Msg {
        match self {
            Self::Setup => msg!("open-ssh-repair-setup"),
            Self::Enable => msg!("open-ssh-repair-enable"),
            Self::Restart => msg!("open-ssh-repair-restart"),
        }
    }

    fn args(self) -> &'static [&'static str] {
        match self {
            Self::Setup => &["setup", "ssh"],
            Self::Enable => &["settings", "set", "feature.ssh", "true"],
            Self::Restart => &["daemon", "restart"],
        }
    }

    pub fn command(self) -> String {
        format!("sbx {}", self.args().join(" "))
    }

    /// client設定の不足を先に観測した場合も、管理者側のgateを越えて修復しない。
    pub fn verify_policy(
        self,
        host: &dyn HostEnvironment,
    ) -> Result<crate::boundary::host::protocol::SshFeature> {
        let feature = sandbox::inspect_ssh_feature(host)?;
        if self == Self::Restart {
            sandbox::require_ssh_feature_enabled(&feature)?;
        }
        Ok(feature)
    }

    /// project lockとsession leaseを手放したあと、承認された操作だけを実行する。
    pub fn execute(
        self,
        host: &dyn HostEnvironment,
        clock: &dyn Clock,
        target: &str,
        restart_required: bool,
    ) -> Result<bool> {
        let feature = self.verify_policy(host)?;
        // 承認を待つあいだに設定やendpointが直っていれば、変更せず再準備する。
        let checked = match self {
            Self::Setup => sandbox::require_ssh_client(host, target),
            Self::Enable => sandbox::require_ssh_feature_enabled(&feature),
            // 明示的な再起動要求は、設定の保存値だけでは解消したと証明できない。
            Self::Restart if restart_required => Err(Self::restart_required()),
            Self::Restart => sandbox::probe_ssh(host, target),
        };
        match checked {
            Ok(()) => return Ok(false),
            Err(error) if Self::for_error(&error) == Some(self) => {}
            Err(error) => return Err(error),
        }
        let requires_restart = self == Self::Enable && feature.requires_restart;
        let spec = CommandSpec::capture("sbx", self.args())
            .env(EnvPolicy::SbxSsh)
            .timeout(TimeoutClass::SandboxLifecycle);
        let outcome = host.run(&spec).map_err(|error| {
            let Error::Diagnostics(mut diagnostics) = error else {
                return error;
            };
            for diagnostic in &mut diagnostics {
                let previous = diagnostic.description.clone();
                diagnostic
                    .facts
                    .push(crate::design::Fact::cause(diagnostic.id.as_str()));
                diagnostic.id = ErrorId::RemoteSshRepairFailed;
                diagnostic.description = msg!("error-remote-ssh-repair-failed");
                diagnostic.remediation = Some(
                    crate::design::Remediation::text(msg!("remediation-remote-ssh-repair-failed"))
                        .explain(previous),
                );
            }
            Error::Diagnostics(diagnostics)
        })?;
        if !outcome.success() {
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::RemoteSshRepairFailed,
                    msg!("error-remote-ssh-repair-failed"),
                )
                .remediation(msg!("remediation-remote-ssh-repair-failed"))
                .external(outcome.failure()),
            ));
        }
        if self == Self::Enable {
            sandbox::require_ssh_feature(host)?;
        }
        if self == Self::Enable && !requires_restart {
            // 設定は通常5秒以内に反映される。再起動を推測で加えず、反映後に再観測する。
            clock.sleep(Duration::from_secs(5));
        }
        Ok(requires_restart)
    }

    pub fn restart_required() -> Error {
        Error::single(
            Diagnostic::new(
                ErrorId::RemoteSshRestartRequired,
                msg!("error-remote-ssh-restart-required"),
            )
            .remediation(
                crate::design::Remediation::text(msg!("remediation-remote-ssh-restart"))
                    .try_run("sbx daemon restart"),
            ),
        )
    }
}
