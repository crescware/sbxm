use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::config::{ConfigLocation, GlobalConfig};
use crate::design::{Fact, ProgressSink};
use crate::diagnostics::Result;
use crate::project::SandboxName;
use crate::support::files::Placement;
use crate::support::inventory::{self, ProjectState};
use crate::support::select::{self, Candidate};
use crate::support::{daemon, generation, provisioning};

use super::{AllReport, ProjectOutcome, ProjectResult, Scope, apply_locked};

/// 登録済みのすべての案件へ、宣言fileを配置する。
///
/// 案件ごとにproject lockを取り、1件ずつ単体の`apply --files`と同じ規則で置く。案件どうしは
/// 独立しているため、1件が置けなくてもほかの案件は続ける。停止中のSandboxは起動せず、
/// Sandboxの無い案件には何もしない。どちらも結果として示す。
pub fn run_all(
    location: &ConfigLocation,
    config: &GlobalConfig,
    force: bool,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
) -> Result<AllReport> {
    let candidates = select::candidates(location)?;
    if candidates.is_empty() {
        return Err(select::no_managed_projects());
    }

    let mut outcomes = Vec::with_capacity(candidates.len());
    let mut failures = Vec::new();
    for candidate in candidates {
        let project = candidate.display_id();
        let sandbox = SandboxName::derive(candidate.repository.canonical_id()).to_string();
        let result = match apply_one(candidate, config, force, host, workspace_root, progress) {
            Ok(result) => result,
            Err(error) => {
                // 同じ宣言fileは案件ごとに同じ配置先を持つ。どの案件の診断かを添える。
                failures.extend(error.diagnostics().iter().cloned().map(|mut diagnostic| {
                    diagnostic.facts.insert(0, Fact::project(&project));
                    diagnostic
                }));
                ProjectResult::Failed
            }
        };
        outcomes.push(ProjectOutcome {
            project,
            sandbox,
            result,
        });
    }
    Ok(AllReport { outcomes, failures })
}

/// 1案件をlockし、動いているSandboxにだけ宣言fileを置く。
fn apply_one(
    candidate: Candidate,
    config: &GlobalConfig,
    force: bool,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
    progress: &mut dyn ProgressSink,
) -> Result<ProjectResult> {
    let locked = candidate.lock()?;
    // Sandboxの状態より先に判定する。中断した初回構築は固定したsnapshotで再開するため、
    // Sandboxが無くても現在の宣言が置かれるとは言えない。
    generation::require_no_rebuild(&locked.metadata)?;
    provisioning::require_no_initial_intent(&locked.metadata)?;
    let entries = daemon::list(host)?;
    match inventory::state_of(&entries, &locked.metadata, workspace_root)? {
        ProjectState::NotCreated => return Ok(ProjectResult::NotCreated),
        ProjectState::Stopped => return Ok(ProjectResult::Stopped),
        ProjectState::Running => {}
    }
    let scope = Scope {
        files: true,
        force,
        worktrees: None,
    };
    let output = apply_locked(locked, config, scope, host, workspace_root, progress)?;
    Ok(
        if output
            .files
            .iter()
            .any(|file| file.placement == Placement::Placed)
        {
            ProjectResult::Applied
        } else {
            ProjectResult::Unchanged
        },
    )
}

#[cfg(test)]
#[path = "run_all_test.rs"]
mod run_all_test;
