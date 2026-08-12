//! bare repositoryとmanaged worktreeの診断。
use crate::command::{CommandOutcome, CommandSpec, HostEnvironment};
use crate::commands::status::project::{ProjectStatus, Value, WorktreeRow};
use crate::design::Fact;
use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;
use crate::project::SandboxLayout;

use crate::testing::outcome::{Checked, Required};

use super::{super::diagnose, super::fake::*};
use crate::testing::host::FakeSbx;
use crate::testing::project::{Fixture, project_id};

#[test]
fn a_running_sandbox_is_looked_into_and_its_worktrees_classified() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = looking_inside(&fixture, &project, &three_entries(&project))?;

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;

    assert_eq!(value_of(&status, "status-item-sandbox")?, Value::Running);
    assert_eq!(value_of(&status, "status-item-secret")?, Value::Ready);
    assert_eq!(
        value_of(&status, "status-item-bare-repository")?,
        Value::Ready
    );
    assert_eq!(
        value_of(&status, "status-item-ssh-agent")?,
        Value::NotExposed
    );
    assert_eq!(
        status.worktrees,
        vec![
            WorktreeRow {
                path: "example-repo.tree-0".to_string(),
                kind: "managed",
                mode: Value::Attached,
                state: Value::Clean,
            },
            WorktreeRow {
                path: "agent-scratch".to_string(),
                kind: "unmanaged",
                mode: Value::Detached,
                state: Value::Dirty,
            },
        ]
    );
    assert_eq!(value_of(&status, "status-item-worktrees")?, Value::Ready);
    assert!(status.diagnostics.is_empty(), "{:?}", status.diagnostics);
    Ok(())
}

#[test]
fn a_worktree_outside_the_shared_repository_is_not_counted_as_the_projects() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let worktrees = format!(
        "{}worktree /work/elsewhere\0detached\0\0",
        three_entries(&project)
    );
    let host = looking_inside(&fixture, &project, &worktrees)?;

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;

    assert_eq!(value_of(&status, "status-item-worktrees")?, Value::Mismatch);
    assert_eq!(
        status
            .worktrees
            .iter()
            .map(|row| row.path.as_str())
            .collect::<Vec<&str>>(),
        vec!["example-repo.tree-0", "agent-scratch"],
        "an outside worktree is not one of the project's"
    );
    assert!(
        status.diagnostics.iter().any(|diagnostic| {
            diagnostic.id == ErrorId::SandboxRepositoryUnusable
                && diagnostic.facts.iter().any(|fact| {
                    matches!(fact, Fact::OneLine { label, value }
                        if label.id == "diagnostic-path-label" && value.as_str() == "/work/elsewhere")
                })
        }),
        "the outside worktree is named: {:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn a_declared_worktree_that_is_missing_is_reported_as_unusable() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let worktrees = format!(
        "worktree {root}\0bare\0\0worktree {root}/agent-scratch\0detached\0\0",
        root = layout.bare_root()
    );
    let host = looking_inside(&fixture, &project, &worktrees)?;

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;

    assert_eq!(value_of(&status, "status-item-worktrees")?, Value::Mismatch);
    assert!(
        status.worktrees.iter().any(|row| {
            row.path == "example-repo.tree-0"
                && row.kind == "managed"
                && row.mode == Value::Mismatch
                && row.state == Value::Mismatch
        }),
        "the missing managed worktree is named: {:?}",
        status.worktrees
    );
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SandboxRepositoryUnusable),
        "the missing managed worktree is diagnosed: {:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn a_repository_check_that_could_not_run_is_not_read_as_missing() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let listing = format!("[{}]", fixture.entry(&project, "running")?);
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let host = FakeSbx::listing(&listing).answering(
        &format!(
            "exec {} -- git --git-dir {} rev-parse --is-bare-repository",
            project.sandbox,
            layout.bare_git_dir()
        ),
        127,
        "",
    );

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;
    assert_eq!(
        value_of(&status, "status-item-bare-repository")?,
        Value::Mismatch
    );

    // repositoryとして扱えないことは、gitが答えた結果なので不在とする。
    let host = FakeSbx::listing(&listing).answering(
        &format!(
            "exec {} -- git --git-dir {} rev-parse --is-bare-repository",
            project.sandbox,
            layout.bare_git_dir()
        ),
        128,
        "",
    );
    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;
    assert_eq!(
        value_of(&status, "status-item-bare-repository")?,
        Value::Missing
    );
    Ok(())
}

#[test]
fn a_repository_check_the_host_could_not_start_stays_the_hosts_own_failure() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    // hostがcommandを起動できなかったことは、repositoryについて何も語らない。
    let host = UnrunnableCommand {
        inner: looking_inside(&fixture, &project, &three_entries(&project))?,
        needle: "rev-parse --is-bare-repository".to_string(),
    };

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;

    assert_eq!(
        value_of(&status, "status-item-bare-repository")?,
        Value::Mismatch
    );
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::ExternalCommandTimeout),
        "the host's own failure is reported as itself: {:?}",
        status.diagnostics
    );
    // 観測できなかったことを、repositoryが壊れているという結論へ言い換えない。
    assert!(
        !status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SandboxRepositoryUnusable),
        "{:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn a_worktree_listing_that_failed_leaves_no_worktree_row_behind() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    // 一覧が読めなかったことと、worktreeが1本も無いことは別である。
    let host = looking_inside(&fixture, &project, &three_entries(&project))?.answering(
        &format!(
            "exec {} -- git --git-dir {} worktree list --porcelain -z",
            project.sandbox,
            layout.bare_git_dir()
        ),
        128,
        "",
    );

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;

    assert_eq!(value_of(&status, "status-item-worktrees")?, Value::Mismatch);
    assert!(
        status.worktrees.is_empty(),
        "an unread listing invents no row: {:?}",
        status.worktrees
    );
    let diagnostic = status
        .diagnostics
        .iter()
        .find(|diagnostic| diagnostic.id == ErrorId::ExternalCommandFailed)
        .required_because("the failed listing is diagnosed")?;
    let external = diagnostic
        .external
        .as_ref()
        .required_because("the original exit status is preserved")?;
    assert!(
        external.exit_status.contains("128"),
        "{:?}",
        external.exit_status
    );
    Ok(())
}

#[test]
fn a_worktree_whose_status_did_not_answer_is_not_reported_as_clean() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    // 変更が無いことは、`git status`が答えた場合にだけ言える。
    let host = looking_inside(&fixture, &project, &three_entries(&project))?.answering(
        &format!(
            "exec {} -- git -C {}/example-repo.tree-0 status --porcelain=v2 -z --untracked-files=all",
            project.sandbox,
            layout.bare_root()
        ),
        128,
        "",
    );

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;

    assert_eq!(state_of(&status, "example-repo.tree-0")?, Value::Mismatch);
    assert_eq!(
        state_of(&status, "agent-scratch")?,
        Value::Dirty,
        "the worktree that did answer keeps its own state"
    );
    assert_eq!(value_of(&status, "status-item-worktrees")?, Value::Mismatch);
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SandboxCheckUnobservable),
        "the check that did not answer is named: {:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn a_status_command_that_could_not_be_run_leaves_the_worktree_unobserved() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    // 内側のcommandが答えなかったことと、hostがcommandを動かせなかったことは別である。
    // どちらもcleanへは丸めず、hostの失敗はそのまま伝える。
    let host = UnrunnableCommand {
        inner: looking_inside(&fixture, &project, &three_entries(&project))?,
        needle: format!("git -C {}/example-repo.tree-0 status", layout.bare_root()),
    };

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
    )
    .required_because("diagnose")?;

    assert_eq!(state_of(&status, "example-repo.tree-0")?, Value::Mismatch);
    assert_eq!(value_of(&status, "status-item-worktrees")?, Value::Mismatch);
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::ExternalCommandTimeout),
        "the host's own failure is reported as itself: {:?}",
        status.diagnostics
    );
    Ok(())
}

/// 行として並んだworktreeの作業状態。
fn state_of(status: &ProjectStatus, path: &str) -> Checked<Value> {
    Ok(status
        .worktrees
        .iter()
        .find(|row| row.path == path)
        .required_because(&format!("worktree {path} is missing"))?
        .state)
}

/// 1つの指定だけを実行できないhost。ほかの指定は`FakeSbx`が答える。
struct UnrunnableCommand {
    inner: FakeSbx,
    needle: String,
}

impl HostEnvironment for UnrunnableCommand {
    fn command_exists(&self, program: &str) -> bool {
        self.inner.command_exists(program)
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        if spec.args.join(" ").contains(&self.needle) {
            return Err(Error::new(
                ErrorId::ExternalCommandTimeout,
                msg!(
                    "error-external-command-timeout",
                    program = spec.program,
                    seconds = 10
                ),
            ));
        }
        self.inner.run(spec)
    }
}
