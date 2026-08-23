use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::project::SandboxLayout;

use crate::design::ProgressSink;
use crate::support::daemon;
use crate::support::inventory::{self, Poll};
use crate::support::protection::{self, DestructiveOperation, ProtectionConfirmation, Request};

use super::{DestroyOutcome, Prepared, finish_removal};

/// 明示確認を経てSandboxを削除する。
///
/// project lockとexclusive session leaseを保持したまま観測を取り直し、`confirmation`が
/// 確認した状態とまだ一致している場合だけ削除する。対象が既に無ければ、削除command
/// を呼ばずに不在をそのまま確定する。
pub fn execute_confirmed(
    host: &dyn HostEnvironment,
    prepared: &Prepared,
    confirmation: ProtectionConfirmation,
    poll: Poll,
    progress: &mut dyn ProgressSink,
) -> Result<DestroyOutcome> {
    let metadata = &prepared.locked.metadata;
    let present = inventory::single(&daemon::list(host)?, prepared.name.as_str())?.is_some();
    let current = if present {
        let layout = SandboxLayout::new(metadata.canonical_id());
        let request = Request::new(
            DestructiveOperation::Destroy,
            &prepared.name,
            &prepared.workspace_root,
            &layout,
            metadata,
        );
        protection::gate::assess(host, &request)?
    } else {
        protection::gate::assess_absent(
            DestructiveOperation::Destroy,
            metadata.display_id(),
            &prepared.name,
        )
    };
    let permit = protection::gate::authorize(confirmation, current)?;

    if present {
        inventory::remove_protected(host, permit, poll, progress)?;
    }
    finish_removal(host, prepared)
}
