use crate::design::Remediation;
use crate::diagnostics::{Error, ErrorId};
use crate::msg;

use super::run::OpenSelection;

/// 接続前に止まった理由へ、同じ対象への再試行とcommit保全の案内を添える。
pub(super) fn connection_error(error: Error, selection: &OpenSelection) -> Error {
    let Error::Diagnostics(mut diagnostics) = error else {
        return error;
    };
    if let Some(diagnostic) = diagnostics.first_mut().filter(|diagnostic| {
        matches!(
            diagnostic.id,
            ErrorId::RemoteSshUnconfigured
                | ErrorId::RemoteSshUnobservable
                | ErrorId::RemoteSshDisabled
                | ErrorId::RemoteSshUnavailable
                | ErrorId::RemoteSshConnectionFailed
                | ErrorId::RemoteSshSettingUnobservable
                | ErrorId::RemoteSshRestricted
                | ErrorId::RemoteSshRepairFailed
                | ErrorId::RemoteSshRestartRequired
        )
    }) {
        let project = selection.candidate.display_id();
        let index = selection
            .index
            .map_or_else(String::new, |index| format!(" -i {index}"));
        let mut remediation = diagnostic
            .remediation
            .take()
            .unwrap_or_else(Remediation::new)
            .explain(msg!("remediation-open-ssh-retry"))
            .try_run(format!("sbxm open {project}{index}"));
        if selection.candidate.repository.host_path().is_some() {
            remediation = remediation
                .explain(msg!("remediation-open-ssh-unsaved"))
                .try_run(format!("sbxm sync {project}"));
        }
        diagnostic.remediation = Some(remediation);
    }
    Error::Diagnostics(diagnostics)
}
