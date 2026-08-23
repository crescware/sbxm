use crate::boundary::host::HostEnvironment;
use crate::compatibility::SandboxEntry;
use crate::diagnostics::Result;
use crate::project::SandboxName;

use crate::support::daemon;

/// 名前が完全一致するSandboxを探す。
///
/// 名前はcanonical project IDから決定的に導出されるため、名前の一致が案件との
/// 対応そのものになる。
pub(super) fn find(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
) -> Result<Option<SandboxEntry>> {
    let sandboxes = daemon::list(host)?;
    Ok(sandboxes
        .into_iter()
        .find(|entry| entry.name == sandbox.as_str()))
}
