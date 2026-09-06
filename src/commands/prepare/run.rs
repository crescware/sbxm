use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::{ConfigLocation, GlobalConfig};
use crate::design::ProgressSink;
use crate::diagnostics::Result;
use crate::msg;
use crate::project::ProjectId;
use crate::support::select::ProjectPrompt;
use crate::support::{generation, provisioning, select};

use super::PrepareOutput;

/// 対象を引数またはpromptで解決し、登録済み案件のSandboxを構築する。
///
/// 観測、状態の分類、構築の順序は`support::provisioning`だけが持つ。この入口は対象の
/// 解決とlockに限り、`open`が同じ初回構築を行うときと別の規則を当てない。
pub fn run(
    location: &ConfigLocation,
    config: &GlobalConfig,
    requested: Option<&ProjectId>,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    prompt: &mut dyn ProjectPrompt,
    progress: &mut dyn ProgressSink,
) -> Result<PrepareOutput> {
    // 対象が決まる前にhostの状態へ触れない。
    let mut locked =
        select::one(location, requested, &msg!("select-prepare-heading"), prompt)?.lock()?;
    generation::require_no_rebuild(&locked.metadata)?;
    provisioning::ensure_initial(&mut locked, config, host, workspace_root, progress)
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;

#[cfg(test)]
#[path = "generation_test.rs"]
mod generation_test;

#[cfg(test)]
#[path = "intent_test.rs"]
mod intent_test;

#[cfg(test)]
#[path = "worktree_test.rs"]
mod worktree_test;

#[cfg(test)]
#[path = "secret_test.rs"]
mod secret_test;

#[cfg(test)]
#[path = "tools_test.rs"]
mod tools_test;

#[cfg(test)]
#[path = "output_test.rs"]
mod output_test;
