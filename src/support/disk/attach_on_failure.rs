use crate::command::{HostEnvironment, TimeoutClass};
use crate::compatibility::SandboxState;
use crate::design::{Fact, Inline};
use crate::diagnostics::Error;
use crate::msg;

use crate::support::inventory::ProjectState;

use super::{DiskObservation, format_gib, format_percent, observe};

/// sbxm自身がSandbox内を変更する工程が失敗したときだけ、失敗直後のSandbox内root
/// filesystemの使用量を`Mount`・`Free`・`Usable`・`Capacity`の`Fact`として追加する。
///
/// 元のErrorId、原因、external stderr、remediationはどれも変えない。観測そのものが
/// 失敗した場合や`Observed`にならなかった場合は、何も足さず元のerrorをそのまま返す。
/// 容量不足と断定しないためである。`Canceled`はここへ来ないが、来た場合も素通りする。
///
/// どの`Diagnostic`も`external`を持たない場合、Sandboxへは1つもcommandを出していない
/// （hostのfile検査など）ため、観測そのものを行わない。原因が既に確定している失敗へ
/// 無関係な空き容量を足し、利用者に容量も関係あるのかと読ませないためである。
///
/// `state`は呼び出し側がmutationを始める前に確認した状態のsnapshotであり、bare clone
/// やfetchが数分走った後の失敗では古くなり得る。その間にSandboxが停止していても
/// `Running`のまま渡ってくるため、「失敗診断のためにstartしない」という約束は、
/// この関数の実装ではなく`sbx exec`が停止中のSandboxを起動しないことに依存する。
///
/// この観測は`TimeoutClass::Probe`（10秒）で行う。容量が尽きたSandboxほど応答が
/// 返らなくなりやすく、`status`/`open`が使う`SandboxLifecycle`（600秒）のままでは、
/// 既に確定している元のerrorの報告が長く遅れてしまうためである。
pub fn attach_on_failure(
    host: &dyn HostEnvironment,
    sandbox: &str,
    state: SandboxState,
    error: Error,
) -> Error {
    let Error::Diagnostics(diagnostics) = &error else {
        return error;
    };
    if diagnostics
        .iter()
        .all(|diagnostic| diagnostic.external.is_none())
    {
        return error;
    }
    let state = match state {
        SandboxState::Running => ProjectState::Running,
        SandboxState::Stopped => ProjectState::Stopped,
    };
    let DiskObservation::Observed(usage) = observe(host, sandbox, Some(state), TimeoutClass::Probe)
    else {
        return error;
    };

    Error::Diagnostics(
        diagnostics
            .iter()
            .cloned()
            .map(|diagnostic| {
                diagnostic
                    .fact(Fact::new(
                        msg!("diagnostic-disk-mount-label"),
                        Inline::path("/"),
                    ))
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
                        Inline::text(format_percent(usage.capacity_percent)),
                    ))
            })
            .collect(),
    )
}
