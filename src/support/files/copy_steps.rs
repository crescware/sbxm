use crate::boundary::host::HostEnvironment;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

use crate::support::sandbox;

use super::{AGENT_HOME, PLACE_FROM_STDIN, TRANSFER_INCOMPLETE};

/// 親directoryを用意し、`bytes`を`sbx exec`のstdinで送って`destination`を置き換える。
pub(super) fn copy_steps(
    host: &dyn HostEnvironment,
    sandbox: &str,
    bytes: Vec<u8>,
    digest: &str,
    destination: &str,
    pending: &str,
) -> Result<()> {
    let parent = destination
        .rsplit_once('/')
        .map_or_else(|| AGENT_HOME.to_string(), |(parent, _)| parent.to_string());
    sandbox::exec_as_root(
        host,
        sandbox,
        &[
            "install", "-d", "-o", "agent", "-g", "agent", "-m", "0700", &parent,
        ],
    )?
    .require_success()?;

    // 置き換えはrenameで行い、読み手へ半端な内容を見せない。
    let outcome = sandbox::exec_as_root_with_input(
        host,
        sandbox,
        &[
            "sh",
            "-c",
            PLACE_FROM_STDIN,
            "sh",
            destination,
            pending,
            digest,
        ],
        bytes,
    )?;
    if sandbox::inner_exit_code(&outcome) == Some(TRANSFER_INCOMPLETE) {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::DeclaredFileTransferIncomplete,
                msg!(
                    "error-declared-file-transfer-incomplete",
                    sandbox = sandbox,
                    destination = destination
                ),
            )
            .remediation(msg!("remediation-declared-file-transfer-incomplete")),
        ));
    }
    outcome.require_success()?;
    Ok(())
}
