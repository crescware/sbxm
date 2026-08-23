use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::compatibility::parse_df;

use crate::support::inventory::ProjectState;
use crate::support::sandbox;

use super::DiskObservation;

/// root filesystemの使用量を観測する。
///
/// Sandboxが動いていない場合は、観測のために起動しない。状態そのものが分からない
/// 場合も同様に、`df`を実行せず理由を返す。
///
/// `timeout`は呼び出し側の事情に合わせる。補助的な観測や診断では、容量が尽きたSandbox
/// ほど応答が返らなくなりやすいため、`Probe`のような短いtimeoutを選ぶ。
pub fn observe(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    state: Option<ProjectState>,
    timeout: TimeoutClass,
) -> DiskObservation {
    match state {
        Some(ProjectState::NotCreated) => return DiskObservation::NotObservedNotCreated,
        Some(ProjectState::Stopped) => return DiskObservation::NotObservedStopped,
        None => return DiskObservation::NotObservedMismatch,
        Some(ProjectState::Running) => {}
    }

    let Ok(outcome) = sandbox::exec_with_timeout(host, sandbox_name, &["df", "-Pk", "/"], timeout)
    else {
        return DiskObservation::ParseFailed;
    };
    match outcome.status.code() {
        // `inner_exit_code`はexec自体の失敗とsignal終了をNoneへまとめるため、ここでは
        // raw statusを使う。127以外の終了理由から`df`不在を推測しない。
        Some(127) => DiskObservation::CommandMissing,
        Some(0) => match parse_df(&outcome.stdout_text()) {
            Ok(usage) => DiskObservation::Observed(usage),
            Err(_) => DiskObservation::ParseFailed,
        },
        Some(_) | None => DiskObservation::ParseFailed,
    }
}
