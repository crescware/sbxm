//! `sbxm stop`。
//!
//! 対象のSandboxを停止する。内部filesystem、Git、package、Docker imageは削除しない。

use std::path::Path;
use std::time::Instant;

use crate::command::{CommandSpec, EnvPolicy, HostEnvironment, TimeoutClass};
use crate::config::GlobalConfig;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::paths::{self, ExclusiveLock, LOCK_TIMEOUT, PRIVATE_FILE_MODE, PathScope};
use crate::project::{ProjectId, SandboxName};

use super::daemon;
use super::inventory::{self, Poll, ProjectState};
use super::select::{self, ProjectPrompt};

/// 1案件の停止結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopResult {
    /// この実行で停止した。
    Stopped,
    /// この実行では停止していない。
    Unchanged,
    /// 停止に失敗した。
    Failed,
}

impl StopResult {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            StopResult::Stopped => "stopped",
            StopResult::Unchanged => "unchanged",
            StopResult::Failed => "failed",
        }
    }

    pub fn legend_id(self) -> &'static str {
        match self {
            StopResult::Stopped => "legend-stopped-now",
            StopResult::Unchanged => "legend-not-stopped",
            StopResult::Failed => "legend-failed",
        }
    }
}

/// 対象1件の結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopOutcome {
    pub project: String,
    pub sandbox: String,
    pub result: StopResult,
}

/// `stop`の結果。
#[derive(Debug, Clone)]
pub struct StopReport {
    pub outcomes: Vec<StopOutcome>,
    /// 失敗した対象の診断。1件でもあればexit code `1`とする。
    pub failures: Vec<Diagnostic>,
}

/// 対象のSandboxを停止する。
///
/// 全対象のvalidationをmutation前に完了し、1件でも進められない状態があれば
/// 何も停止しない。停止の途中で失敗した場合は、後続の対象を停止しない。
pub fn run(
    config: &GlobalConfig,
    requested: &[ProjectId],
    host: &dyn HostEnvironment,
    prompt: &mut dyn ProjectPrompt,
    workspace_root: &Path,
    poll: Poll,
) -> Result<StopReport> {
    // 1. 全対象のmetadataを解決する。canonical ID昇順で返る。
    let selected = select::many(config, requested, prompt)?;

    // 2-3. 1回の一覧取得で全stateを解決し、進められない状態が1件でもあれば止める。
    let entries = daemon::list(host)?;
    for candidate in &selected {
        validate(&candidate.metadata, &entries, workspace_root)?;
    }

    // 4. 複数lockはcanonical ID昇順に取得する。
    let lock_paths: Vec<std::path::PathBuf> = selected
        .iter()
        .map(|candidate| candidate.paths.lock_file())
        .collect();
    let mut locks: Vec<ExclusiveLock> = Vec::with_capacity(lock_paths.len());
    for path in &lock_paths {
        locks.push(paths::acquire_exclusive_lock(
            path,
            LOCK_TIMEOUT,
            PRIVATE_FILE_MODE,
            PathScope::ProjectPath,
        )?);
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
            match stop_one(host, &target.sandbox, poll) {
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

/// 停止対象1件。
struct Target {
    display_id: String,
    sandbox: SandboxName,
    state: ProjectState,
}

impl Target {
    fn outcome(&self, result: StopResult) -> StopOutcome {
        StopOutcome {
            project: self.display_id.clone(),
            sandbox: self.sandbox.as_str().to_string(),
            result,
        }
    }
}

/// 停止して良い状態であることを確かめ、現在のstateを返す。
fn validate(
    metadata: &ProjectMetadata,
    entries: &[crate::compatibility::SandboxEntry],
    workspace_root: &Path,
) -> Result<ProjectState> {
    require_no_rebuild(metadata)?;
    inventory::state_of(entries, metadata, workspace_root)
}

fn require_no_rebuild(metadata: &ProjectMetadata) -> Result<()> {
    if metadata.rebuild.is_none() {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::RebuildIntentPending,
            msg!(
                "error-rebuild-intent-pending",
                project = metadata.display_id()
            ),
        )
        .remediation(msg!(
            "remediation-run-rebuild",
            command = format!("sbxm rebuild {}", metadata.display_id())
        )),
    ))
}

/// 1件を停止し、stoppedになるまで待つ。
fn stop_one(host: &dyn HostEnvironment, sandbox: &SandboxName, poll: Poll) -> Result<()> {
    let spec = CommandSpec::passthrough("sbx", &["stop", sandbox.as_str()])
        .env(EnvPolicy::InheritWithoutSshAgent)
        .timeout(TimeoutClass::SandboxLifecycle);
    host.run(&spec)?.require_success()?;

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
mod tests {
    use super::*;
    use crate::command::OutputPolicy;
    use crate::metadata::{self, RebuildIntent};
    use crate::workflow::inventory::tests::{FakeSbx, fixture};
    use crate::workflow::select::tests::ScriptedPrompt;
    use std::time::Duration;

    fn poll() -> Poll {
        Poll {
            interval: Duration::from_millis(1),
            limit: Duration::from_millis(20),
        }
    }

    fn project_id(value: &str) -> ProjectId {
        ProjectId::parse(value).expect("valid project id")
    }

    #[test]
    fn only_the_running_targets_are_stopped() {
        let fixture = fixture();
        let first = fixture.register("alpha/repo");
        let second = fixture.register("zeta/repo");
        let running = format!(
            "[{},{}]",
            fixture.entry(&first, "running"),
            fixture.entry(&second, "stopped")
        );
        let after = format!(
            "[{},{}]",
            fixture.entry(&first, "stopped"),
            fixture.entry(&second, "stopped")
        );
        let host = FakeSbx::listings(&[&running, &running, &after]);

        let report = run(
            &fixture.config,
            &[project_id("zeta/repo"), project_id("alpha/repo")],
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
            poll(),
        )
        .expect("stop");

        assert_eq!(
            report
                .outcomes
                .iter()
                .map(|outcome| (outcome.project.as_str(), outcome.result))
                .collect::<Vec<_>>(),
            vec![
                ("alpha/repo", StopResult::Stopped),
                ("zeta/repo", StopResult::Unchanged),
            ],
            "targets are processed in canonical order"
        );
        assert!(report.failures.is_empty());
        assert!(host.ran(&format!("stop {}", first.sandbox)));
        assert!(
            !host.ran(&format!("stop {}", second.sandbox)),
            "a sandbox that is already stopped is left alone"
        );

        let stop = host.spec(&format!("stop {}", first.sandbox));
        assert_eq!(stop.output, OutputPolicy::Passthrough);
        assert_eq!(stop.env, EnvPolicy::InheritWithoutSshAgent);
        assert_eq!(
            host.spec("ls --json").output,
            OutputPolicy::Capture,
            "the state is read from structured output"
        );
    }

    #[test]
    fn a_project_without_a_sandbox_is_a_no_op_success() {
        let fixture = fixture();
        fixture.register("example-org/example-repo");
        let host = FakeSbx::listing("[]");

        let report = run(
            &fixture.config,
            &[project_id("example-org/example-repo")],
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
            poll(),
        )
        .expect("stop");
        assert_eq!(report.outcomes[0].result, StopResult::Unchanged);
        assert!(!host.ran("stop "));
    }

    #[test]
    fn a_rebuild_in_progress_stops_nothing_at_all() {
        let fixture = fixture();
        let first = fixture.register("alpha/repo");
        let second = fixture.register("zeta/repo");
        let mut metadata = second.metadata.clone();
        metadata.rebuild = Some(RebuildIntent {
            target_dockerfile_sha256: "2".repeat(64),
            previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
        });
        metadata::update(&second.paths, &metadata).expect("record the intent");

        let listing = format!(
            "[{},{}]",
            fixture.entry(&first, "running"),
            fixture.entry(&second, "running")
        );
        let host = FakeSbx::listing(&listing);

        let error = run(
            &fixture.config,
            &[project_id("alpha/repo"), project_id("zeta/repo")],
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
            poll(),
        )
        .expect_err("one target that cannot be stopped stops the whole run");
        assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
        assert!(!host.ran("stop "), "nothing is stopped: {:?}", host.calls());
    }

    #[test]
    fn an_intent_recorded_after_the_first_check_is_still_seen() {
        let fixture = fixture();
        let project = fixture.register("alpha/repo");
        let listing = format!("[{}]", fixture.entry(&project, "running"));
        let host = FakeSbx::listing(&listing);

        // 選択と最初の検査のあとにrebuildが始まった状態を、lock取得後に読み直す。
        let mut metadata = project.metadata.clone();
        metadata.rebuild = Some(RebuildIntent {
            target_dockerfile_sha256: "2".repeat(64),
            previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
        });
        metadata::update(&project.paths, &metadata).expect("record the intent");

        let error = run(
            &fixture.config,
            &[project_id("alpha/repo")],
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
            poll(),
        )
        .expect_err("the metadata on disk decides after the lock is held");
        assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
        assert!(!host.ran("stop "));
    }

    #[test]
    fn a_failure_leaves_the_remaining_targets_running() {
        let fixture = fixture();
        let first = fixture.register("alpha/repo");
        let second = fixture.register("zeta/repo");
        let running = format!(
            "[{},{}]",
            fixture.entry(&first, "running"),
            fixture.entry(&second, "running")
        );
        let host = FakeSbx::listing(&running).answering(&format!("stop {}", first.sandbox), 1, "");

        let report = run(
            &fixture.config,
            &[project_id("alpha/repo"), project_id("zeta/repo")],
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
            poll(),
        )
        .expect("the report is produced even when a target fails");

        assert_eq!(report.outcomes[0].result, StopResult::Failed);
        assert_eq!(report.outcomes[1].result, StopResult::Unchanged);
        assert!(!report.failures.is_empty());
        assert!(
            !host.ran(&format!("stop {}", second.sandbox)),
            "the run does not continue past a failure"
        );
    }

    #[test]
    fn a_sandbox_that_stays_running_is_reported_as_failed() {
        let fixture = fixture();
        let project = fixture.register("example-org/example-repo");
        let running = format!("[{}]", fixture.entry(&project, "running"));
        let host = FakeSbx::listing(&running);

        let report = run(
            &fixture.config,
            &[project_id("example-org/example-repo")],
            &host,
            &mut ScriptedPrompt::choosing(0),
            &fixture.workspace_root,
            poll(),
        )
        .expect("report");
        assert_eq!(report.outcomes[0].result, StopResult::Failed);
        assert!(
            report
                .failures
                .iter()
                .any(|diagnostic| diagnostic.id == ErrorId::SandboxStillRunning)
        );
    }

    #[test]
    fn an_omitted_target_is_chosen_from_the_managed_projects() {
        let fixture = fixture();
        let first = fixture.register("alpha/repo");
        let second = fixture.register("zeta/repo");
        let running = format!(
            "[{},{}]",
            fixture.entry(&first, "running"),
            fixture.entry(&second, "running")
        );
        let after = format!(
            "[{},{}]",
            fixture.entry(&first, "stopped"),
            fixture.entry(&second, "running")
        );
        let host = FakeSbx::listings(&[&running, &running, &after]);

        let report = run(
            &fixture.config,
            &[],
            &host,
            &mut ScriptedPrompt::choosing_many(&[0]),
            &fixture.workspace_root,
            poll(),
        )
        .expect("stop");
        assert_eq!(report.outcomes.len(), 1);
        assert_eq!(report.outcomes[0].project, "alpha/repo");
    }
}
