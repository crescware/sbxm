use std::path::Path;
use std::time::Instant;

use crate::command::{EnvPolicy, HostEnvironment, TerminalCommand, TimeoutClass};
use crate::config::ConfigLocation;
use crate::design::ExternalOutput;
use crate::diagnostics::{Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::paths::ExclusiveLock;
use crate::project::{ProjectId, SandboxName};

use crate::support::inventory::{self, Poll, ProjectState};
use crate::support::select::{self, ProjectPrompt};
use crate::support::{daemon, generation};

use super::{StopReport, StopResult, Target};

/// 対象のSandboxを停止する。
///
/// 全対象のvalidationをmutation前に完了し、1件でも進められない状態があれば
/// 何も停止しない。停止の途中で失敗した場合は、後続の対象を停止しない。
pub fn run(
    location: &ConfigLocation,
    requested: &[ProjectId],
    host: &dyn HostEnvironment,
    prompt: &mut dyn ProjectPrompt,
    workspace_root: &Path,
    poll: Poll,
    output: &mut dyn ExternalOutput,
) -> Result<StopReport> {
    // 1. 全対象のmetadataを解決する。canonical ID昇順で返る。
    let selected = select::many(location, requested, &msg!("select-stop-heading"), prompt)?;

    // 2-3. 1回の一覧取得で全stateを解決し、進められない状態が1件でもあれば止める。
    let entries = daemon::list(host)?;
    for candidate in &selected {
        validate(&candidate.reload()?, &entries, workspace_root)?;
    }

    // 4. 複数lockはcanonical ID昇順に取得する。
    let mut locks: Vec<ExclusiveLock> = Vec::with_capacity(selected.len());
    for candidate in &selected {
        locks.push(candidate.paths.acquire_lock()?);
    }

    // 5. lock取得後のmetadataとstateでpreconditionを判定し直す。
    let entries = daemon::list(host)?;
    let mut targets: Vec<Target> = Vec::with_capacity(selected.len());
    for candidate in &selected {
        let metadata = candidate.reload()?;
        let state = validate(&metadata, &entries, workspace_root)?;
        targets.push(Target {
            display_id: metadata.display_id(),
            sandbox: metadata.sandbox_name(),
            state,
        });
    }

    // 6. runningだけを停止する。
    let mut outcomes = Vec::with_capacity(targets.len());
    let mut failures = Vec::new();
    for target in &targets {
        // 失敗した時点で、後続の対象は停止せずそのままにする。
        if failures.is_empty() && target.state == ProjectState::Running {
            match stop_one(host, &target.sandbox, poll, output) {
                Ok(()) => outcomes.push(target.outcome(StopResult::Stopped)),
                Err(error) => {
                    failures.extend(error.diagnostics().iter().cloned());
                    outcomes.push(target.outcome(StopResult::Failed));
                }
            }
        } else {
            outcomes.push(target.outcome(StopResult::Unchanged));
        }
    }

    drop(locks);
    Ok(StopReport { outcomes, failures })
}

/// 停止して良い状態であることを確かめ、現在のstateを返す。
fn validate(
    metadata: &ProjectMetadata,
    entries: &[crate::compatibility::SandboxEntry],
    workspace_root: &Path,
) -> Result<ProjectState> {
    generation::require_no_rebuild(metadata)?;
    inventory::state_of(entries, metadata, workspace_root)
}

/// 1件を停止し、stoppedになるまで待つ。
fn stop_one(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
    poll: Poll,
    output: &mut dyn ExternalOutput,
) -> Result<()> {
    let command = TerminalCommand::relayed("sbx", &["stop", sandbox.as_str()])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    host.run_with_terminal(&command, output)?
        .require_success()?;

    let deadline = Instant::now() + poll.limit;
    loop {
        let entries = daemon::list(host)?;
        let state = entries
            .iter()
            .find(|entry| entry.name == sandbox.as_str())
            .map(|entry| entry.state);
        match state {
            // 削除されている場合も、起動していないことは確認できている。
            None | Some(crate::compatibility::SandboxState::Stopped) => return Ok(()),
            Some(crate::compatibility::SandboxState::Running) if Instant::now() >= deadline => {
                return Err(Error::new(
                    ErrorId::SandboxStillRunning,
                    msg!("error-sandbox-still-running", sandbox = sandbox),
                ));
            }
            Some(crate::compatibility::SandboxState::Running) => std::thread::sleep(poll.interval),
        }
    }
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;
