use crate::command::HostEnvironment;
use crate::compatibility::parse_df;

use crate::support::inventory::ProjectState;
use crate::support::sandbox;

use super::DiskObservation;

/// root filesystemの使用量を観測する。
///
/// Sandboxが動いていない場合は、観測のために起動しない。状態そのものが分からない
/// 場合も同様に、`df`を実行せず理由を返す。
pub fn observe(
    host: &dyn HostEnvironment,
    sandbox_name: &str,
    state: Option<ProjectState>,
) -> DiskObservation {
    match state {
        Some(ProjectState::NotCreated) => return DiskObservation::NotObservedNotCreated,
        Some(ProjectState::Stopped) => return DiskObservation::NotObservedStopped,
        None => return DiskObservation::NotObservedMismatch,
        Some(ProjectState::Running) => {}
    }

    let Ok(outcome) = sandbox::exec(host, sandbox_name, &["df", "-Pk", "/"]) else {
        return DiskObservation::ParseFailed;
    };
    match sandbox::inner_exit_code(&outcome) {
        // `sbx exec`はcommandを起動できなかったことを125..=127で示す。`df`のような
        // 単純な起動には他に理由が無いため、起動できなかったことを不在として読む。
        None => DiskObservation::CommandMissing,
        Some(0) => match parse_df(&outcome.stdout_text()) {
            Ok(usage) => DiskObservation::Observed(usage),
            Err(_) => DiskObservation::ParseFailed,
        },
        Some(_) => DiskObservation::ParseFailed,
    }
}
