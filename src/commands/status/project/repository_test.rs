//! bare repositoryとmanaged worktreeの診断。
use super::super::diagnose;
use super::super::fake::*;
use super::*;
use crate::testing::host::FakeSbx;
use crate::testing::project::{fixture, project_id};

#[test]
fn a_running_sandbox_is_looked_into_and_its_worktrees_classified() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let host = looking_inside(&fixture, &project, &three_entries(&project));

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");

    assert_eq!(value_of(&status, "status-item-sandbox"), Value::Running);
    assert_eq!(value_of(&status, "status-item-secret"), Value::Ready);
    assert_eq!(
        value_of(&status, "status-item-bare-repository"),
        Value::Ready
    );
    assert_eq!(
        value_of(&status, "status-item-ssh-agent"),
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
    assert_eq!(value_of(&status, "status-item-worktrees"), Value::Ready);
    assert!(status.diagnostics.is_empty(), "{:?}", status.diagnostics);
}

#[test]
fn a_worktree_outside_the_shared_repository_is_not_counted_as_the_projects() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let worktrees = format!(
        "{}worktree /work/elsewhere\0detached\0\0",
        three_entries(&project)
    );
    let host = looking_inside(&fixture, &project, &worktrees);

    let status = diagnose(
        &fixture.location,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");

    assert_eq!(value_of(&status, "status-item-worktrees"), Value::Mismatch);
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
                && diagnostic
                    .description
                    .args
                    .contains(&("path", "/work/elsewhere".to_string()))
        }),
        "the outside worktree is named: {:?}",
        status.diagnostics
    );
}

#[test]
fn a_repository_check_that_could_not_run_is_not_read_as_missing() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let listing = format!("[{}]", fixture.entry(&project, "running"));
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
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");
    assert_eq!(
        value_of(&status, "status-item-bare-repository"),
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
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");
    assert_eq!(
        value_of(&status, "status-item-bare-repository"),
        Value::Missing
    );
}
