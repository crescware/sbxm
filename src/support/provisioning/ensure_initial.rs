use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::GlobalConfig;
use crate::design::ProgressSink;
use crate::diagnostics::Result;
use crate::support::select::Locked;

use super::{
    InitialRoute, ProvisioningOutput, build_initial, observe, ready_output, resume_initial,
};

/// lock済みの案件を、初回構築が済んだ状態にする共有入口。
///
/// `prepare`も`open`もここを通り、同じ観測から同じ規則で道を決める。既に目標構成が
/// 揃っている案件を作り直さず、中断した初回構築は固定済み入力から再開する。
pub(crate) fn ensure_initial(
    locked: &mut Locked,
    config: &GlobalConfig,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
) -> Result<ProvisioningOutput> {
    let observation = observe(
        host,
        &locked.paths,
        config,
        &locked.metadata,
        workspace_root,
    )?;
    match InitialRoute::decide(&locked.metadata, &observation)? {
        InitialRoute::AlreadyBuilt => Ok(ready_output(&locked.metadata, &observation)),
        InitialRoute::Build => build_initial(locked, config, host, workspace_root, progress),
        InitialRoute::Resume => {
            resume_initial(locked, config, host, workspace_root, progress, &observation)
        }
    }
}
