use crate::command::HostEnvironment;
use crate::compatibility::SandboxState;
use crate::design::{Fact, Inline};
use crate::diagnostics::Error;
use crate::msg;

use crate::support::inventory::ProjectState;

use super::{DiskObservation, format_gib, observe};

/// sbxm自身がSandbox内を変更する工程が失敗したときだけ、失敗直後の空き容量を追加の
/// Factとして載せる。
///
/// 元のErrorId、原因、external stderr、remediationはどれも変えない。観測そのものが
/// 失敗した場合や`Observed`にならなかった場合は、何も足さず元のerrorをそのまま返す。
/// 容量不足と断定しないためである。`Canceled`はここへ来ないが、来た場合も素通りする。
pub fn attach_on_failure(
    host: &dyn HostEnvironment,
    sandbox: &str,
    state: SandboxState,
    error: Error,
) -> Error {
    let Error::Diagnostics(diagnostics) = &error else {
        return error;
    };
    let state = match state {
        SandboxState::Running => ProjectState::Running,
        SandboxState::Stopped => ProjectState::Stopped,
    };
    let DiskObservation::Observed(usage) = observe(host, sandbox, Some(state)) else {
        return error;
    };

    Error::Diagnostics(
        diagnostics
            .iter()
            .cloned()
            .map(|diagnostic| {
                diagnostic
                    .fact(Fact::new(
                        msg!("diagnostic-disk-free-label"),
                        Inline::text(format_gib(usage.free_kib)),
                    ))
                    .fact(Fact::new(
                        msg!("diagnostic-disk-usable-label"),
                        Inline::text(format_gib(usage.usable_kib)),
                    ))
                    .fact(Fact::new(
                        msg!("diagnostic-disk-capacity-label"),
                        Inline::text(format!("{}%", usage.capacity_percent)),
                    ))
            })
            .collect(),
    )
}
