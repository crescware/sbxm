use crate::boundary::host::HostEnvironment;
use crate::diagnostics::ErrorId;
use crate::support::{StatusValue, sandbox};

use crate::commands::status::global::{GlobalStatus, push};

/// 設定の有効性だけを示す。daemonもSandboxも、この検査のために起動しない。
pub(super) fn check_ssh_endpoint(
    host: &dyn HostEnvironment,
    daemon: StatusValue,
    status: &mut GlobalStatus,
) {
    if daemon != StatusValue::Running {
        push(status, "status-item-ssh-endpoint", daemon);
        return;
    }
    match sandbox::require_ssh_feature(host) {
        Ok(()) => push(status, "status-item-ssh-endpoint", StatusValue::Ready),
        Err(error) => {
            let value = if error.contains_id(ErrorId::RemoteSshDisabled)
                || error.contains_id(ErrorId::RemoteSshRestricted)
            {
                StatusValue::Missing
            } else {
                StatusValue::Error
            };
            push(status, "status-item-ssh-endpoint", value);
            status.diagnostics.extend_from_slice(error.diagnostics());
        }
    }
}
