use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::GlobalConfig;
use crate::design::ProgressSink;
use crate::diagnostics::Result;
use crate::msg;
use crate::support::select;

use super::{ApplyOutput, Scope, Target, apply_locked};

/// 対象を引数またはpromptで解決し、構築済みの案件へ変更を適用する。
///
/// Sandboxの中身を変えるmutationであるため、対象を確かめた後にproject lockを取得し、
/// lock取得後のmetadataでpreconditionを判定し直してから適用する。
pub fn run(
    target: Target,
    config: &GlobalConfig,
    scope: Scope,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
) -> Result<ApplyOutput> {
    let Target {
        location,
        requested,
        prompt,
    } = target;
    // 対象が決まる前にhostの状態へ触れない。
    let locked = select::one(location, requested, &msg!("select-apply-heading"), prompt)?.lock()?;
    apply_locked(locked, config, scope, host, workspace_root, progress)
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;
