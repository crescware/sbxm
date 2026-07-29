use super::*;
use crate::project::SandboxLayout;
use crate::support::image;
use crate::testing::host::{FakeSbx, isolated_agent, registered_secret};
use crate::testing::project::{Fixture, Registered, fixture, project_id};

fn value_of(status: &ProjectStatus, item: &str) -> Value {
    status
        .items
        .iter()
        .find(|entry| entry.item == item)
        .unwrap_or_else(|| panic!("item {item} is missing"))
        .value
}

/// imageがまだ存在しないhost。一覧は答えるが、1件も返さない。
fn without_image(host: FakeSbx, project: &Registered) -> FakeSbx {
    let image = image::image_name(
        &project.sandbox,
        &project.metadata.provisioning.dockerfile_sha256,
    );
    host.answering(&format!("image ls --quiet {image}"), 0, "")
}

#[test]
fn a_project_that_is_not_managed_cannot_be_diagnosed() {
    let fixture = fixture();
    let host = FakeSbx::listing("[]");
    let error = diagnose(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect_err("there is nothing to diagnose");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));
}

#[test]
fn the_items_are_reported_in_the_documented_order() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let host = without_image(FakeSbx::listing("[]"), &project);

    let status = diagnose(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");

    assert_eq!(
        status
            .items
            .iter()
            .map(|item| item.item)
            .collect::<Vec<_>>(),
        vec![
            "status-item-metadata",
            "status-item-project-root",
            "status-item-host-clone",
            "status-item-dockerfile",
            "status-item-image",
            "status-item-template-archive",
            "status-item-sandbox",
            "status-item-workspace",
            "status-item-secret",
            "status-item-bare-repository",
            "status-item-worktrees",
            "status-item-ssh-agent",
        ]
    );
}

#[test]
fn a_project_without_a_sandbox_reports_the_inner_items_as_not_applicable() {
    let fixture = fixture();
    let project = fixture.register("Example-Org/Example-Repo");
    let host = without_image(FakeSbx::listing("[]"), &project);

    let status = diagnose(
        &fixture.config,
        &project_id("Example-Org/Example-Repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");

    assert_eq!(status.project, "Example-Org/Example-Repo");
    assert_eq!(value_of(&status, "status-item-metadata"), Value::Ready);
    assert_eq!(value_of(&status, "status-item-sandbox"), Value::NotCreated);
    for item in [
        "status-item-secret",
        "status-item-bare-repository",
        "status-item-worktrees",
        "status-item-ssh-agent",
    ] {
        assert_eq!(value_of(&status, item), Value::NotApplicable, "{item}");
    }
    assert!(status.worktrees.is_empty());
}

#[test]
fn a_stopped_sandbox_is_not_started_to_look_inside_it() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let host = without_image(
        FakeSbx::listing(&format!("[{}]", fixture.entry(&project, "stopped"))),
        &project,
    );

    let status = diagnose(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");

    assert_eq!(value_of(&status, "status-item-sandbox"), Value::Stopped);
    assert_eq!(
        value_of(&status, "status-item-ssh-agent"),
        Value::NotObservedStopped
    );
    assert_eq!(
        value_of(&status, "status-item-worktrees"),
        Value::NotObservedStopped
    );
    assert!(
        !host.ran("exec"),
        "nothing runs inside a stopped sandbox: {:?}",
        host.calls()
    );
    assert!(
        status.is_healthy(),
        "not observing on purpose is not a failure"
    );
}

#[test]
fn a_sandbox_state_that_cannot_be_read_is_not_reported_as_a_missing_sandbox() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    // 一覧を読めない状態でも、取得できた項目は表示する。
    let host = without_image(FakeSbx::listing("not json"), &project);

    let status = diagnose(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");

    assert_eq!(value_of(&status, "status-item-metadata"), Value::Ready);
    assert_eq!(value_of(&status, "status-item-sandbox"), Value::Mismatch);
    for item in [
        "status-item-secret",
        "status-item-bare-repository",
        "status-item-worktrees",
        "status-item-ssh-agent",
    ] {
        assert_eq!(
            value_of(&status, item),
            Value::Mismatch,
            "{item} is not observed, which is not the same as absent"
        );
    }
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::GlobalScopeUnobservable),
        "the global command that can diagnose this is named: {:?}",
        status.diagnostics
    );
    assert!(!host.ran("exec"), "nothing runs inside an unknown sandbox");
}

#[test]
fn an_unrelated_project_does_not_decide_this_one() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    // 別案件のmetadataが壊れていても、この案件の状態は読める。
    let broken = fixture
        .config
        .base_path
        .as_path()
        .join("broken/broken.project/.sbxm");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join("project.toml"), "version = 2\n").unwrap();

    let host = without_image(
        FakeSbx::listing(&format!("[{}]", fixture.entry(&project, "stopped"))),
        &project,
    );
    let status = diagnose(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");
    assert_eq!(value_of(&status, "status-item-sandbox"), Value::Stopped);
}

#[test]
fn an_engine_that_cannot_be_asked_does_not_make_an_image_absent() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let host = without_image(FakeSbx::listing("[]"), &project);

    let status = diagnose(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");
    assert_eq!(value_of(&status, "status-item-image"), Value::Missing);
    assert!(
        status.diagnostics.is_empty(),
        "an image that is simply not there is not a failure: {:?}",
        status.diagnostics
    );

    let image = image::image_name(
        &project.sandbox,
        &project.metadata.provisioning.dockerfile_sha256,
    );
    let host = FakeSbx::listing("[]").answering(&format!("image ls --quiet {image}"), 1, "");

    let status = diagnose(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");
    assert_eq!(value_of(&status, "status-item-image"), Value::Mismatch);
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::GlobalScopeUnobservable),
        "the engine that could not answer is named: {:?}",
        status.diagnostics
    );
}

#[test]
fn a_changed_dockerfile_is_reported_as_the_next_rebuild_rather_than_a_fault() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();
    let host = without_image(FakeSbx::listing("[]"), &project);

    let status = diagnose(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");
    assert_eq!(value_of(&status, "status-item-dockerfile"), Value::Changed);
    assert!(status.is_healthy(), "a changed Dockerfile is not an error");
}

/// 中まで見られる稼働中Sandbox。`worktrees`は`worktree list --porcelain -z`の答え。
fn looking_inside(fixture: &Fixture, project: &Registered, worktrees: &str) -> FakeSbx {
    let layout = SandboxLayout::new(&project.metadata.canonical_id);
    let host = FakeSbx::listing(&format!("[{}]", fixture.entry(project, "running")))
        .answering(
            &format!(
                "exec {} -- git --git-dir {} rev-parse --is-bare-repository",
                project.sandbox,
                layout.bare_git_dir()
            ),
            0,
            "true\n",
        )
        .answering(
            &format!(
                "exec {} -- git --git-dir {} worktree list --porcelain -z",
                project.sandbox,
                layout.bare_git_dir()
            ),
            0,
            worktrees,
        )
        .answering(
            &format!(
                "exec {} -- git -C {}/agent-scratch status --porcelain=v2 -z --untracked-files=all",
                project.sandbox,
                layout.bare_root()
            ),
            0,
            "1 .M N... 100644 100644 100644 abc abc file.txt\0",
        );
    isolated_agent(
        registered_secret(host, project.sandbox.as_str()),
        project.sandbox.as_str(),
    )
}

/// bare entryと、管理下・管理外のworktreeを1件ずつ並べたporcelain出力。
fn three_entries(project: &Registered) -> String {
    let layout = SandboxLayout::new(&project.metadata.canonical_id);
    format!(
        "worktree {root}\0bare\0\0worktree {root}/example-repo.tree-0\0branch refs/heads/main\0\0worktree {root}/agent-scratch\0detached\0\0",
        root = layout.bare_root()
    )
}

#[test]
fn a_running_sandbox_is_looked_into_and_its_worktrees_classified() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let host = looking_inside(&fixture, &project, &three_entries(&project));

    let status = diagnose(
        &fixture.config,
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
        &fixture.config,
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
fn an_ssh_agent_inside_the_sandbox_is_a_security_failure() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let listing = format!("[{}]", fixture.entry(&project, "running"));
    let host = FakeSbx::listing(&listing)
        .answering(
            &format!("exec {} -- printenv SSH_AUTH_SOCK", project.sandbox),
            0,
            "/tmp/ssh-agent.sock\n",
        )
        .answering(&format!("exec {} -- ssh-add -L", project.sandbox), 2, "");

    let status = diagnose(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");

    assert_eq!(value_of(&status, "status-item-ssh-agent"), Value::Exposed);
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SshAgentExposed)
    );
    assert!(!status.is_healthy());
}

#[test]
fn an_agent_that_answers_without_keys_is_still_reachable() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let listing = format!("[{}]", fixture.entry(&project, "running"));
    // socketは未設定でも、agentへ接続できる時点で露出している。
    let host = FakeSbx::listing(&listing)
        .answering(
            &format!("exec {} -- printenv SSH_AUTH_SOCK", project.sandbox),
            1,
            "",
        )
        .answering(&format!("exec {} -- ssh-add -L", project.sandbox), 1, "");

    let status = diagnose(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");
    assert_eq!(value_of(&status, "status-item-ssh-agent"), Value::Exposed);
}

#[test]
fn a_check_that_could_not_run_is_not_read_as_not_exposed() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let listing = format!("[{}]", fixture.entry(&project, "running"));
    // command不在は、露出していないことの証明にならない。
    let host = FakeSbx::listing(&listing)
        .answering(
            &format!("exec {} -- printenv SSH_AUTH_SOCK", project.sandbox),
            127,
            "",
        )
        .answering(&format!("exec {} -- ssh-add -L", project.sandbox), 127, "");

    let status = diagnose(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
    )
    .expect("diagnose");
    assert_eq!(value_of(&status, "status-item-ssh-agent"), Value::Mismatch);
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::SandboxCheckUnobservable)
    );
    assert!(!status.is_healthy(), "an unprovable check is not a pass");
}

#[test]
fn a_repository_check_that_could_not_run_is_not_read_as_missing() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let listing = format!("[{}]", fixture.entry(&project, "running"));
    let layout = SandboxLayout::new(&project.metadata.canonical_id);
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
        &fixture.config,
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
        &fixture.config,
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
