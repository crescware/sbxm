use crate::diagnostics::{ErrorId, Result};
use crate::project::{ProjectId, SandboxLayout};

use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::recorded_output::RecordedOutput;

use super::*;
use crate::command::{OutputPolicy, TimeoutClass};
use crate::design::SilentProgress;
use crate::metadata::{self, MAX_WORKTREE_INDEX, RebuildIntent};
use crate::paths::{self, PRIVATE_FILE_MODE, PathScope};
use crate::testing::host::{FakeSbx, assert_lifecycle, isolated_agent};
use crate::testing::poll::poll;
use crate::testing::project::{Fixture, Registered, project_id};
use crate::testing::prompt::ScriptedPrompt;
use std::fmt::Write as _;
use std::time::Duration;

/// Docker疎通とworktree一覧に応答するhost。
fn ready(host: FakeSbx, project: &Registered) -> FakeSbx {
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let mut listing = format!("worktree {}\0bare\0\0", layout.bare_root());
    for index in 0..project.metadata.provisioning.requested_worktrees {
        let _ = write!(
            listing,
            "worktree {}/{}\0branch refs/heads/main\0\0",
            layout.bare_root(),
            layout.worktree_name(index)
        );
    }
    let host = isolated_agent(
        host.answering("version --format {{.Server.Version}}", 0, "27.0.3\n"),
        project.sandbox.as_str(),
    );
    host.answering(
        &format!(
            "exec {} -- git --git-dir {} worktree list --porcelain -z",
            project.sandbox,
            layout.bare_git_dir()
        ),
        0,
        &listing,
    )
}

fn prepare_for(fixture: &Fixture, host: &FakeSbx) -> Result<Prepared> {
    prepare(
        &fixture.location,
        None,
        None,
        host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
}

fn prepare_for_index(fixture: &Fixture, host: &FakeSbx, index: Option<u32>) -> Result<Prepared> {
    prepare(
        &fixture.location,
        None,
        index,
        host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
}

#[test]
fn a_running_project_is_opened_without_touching_the_daemon() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("Example-Org/Example-Repo")?;
    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let host = ready(FakeSbx::listing(&running), &project);

    let prepared = prepare_for(&fixture, &host).required_because("prepare")?;

    assert_eq!(prepared.ssh_host, format!("{}.sbx", project.sandbox));
    assert_eq!(
        prepared.working_directory,
        "/home/agent/work/example-repo/example-repo.tree-0"
    );
    assert_eq!(prepared.missing_worktree_index, None);
    // daemonを止めるには動作中のSandboxを止める必要があり、接続のたびに
    // ほかの作業を巻き込むことになる。sbxmはdaemonを操作しない。
    assert!(
        !host.ran("daemon stop") && !host.ran("daemon start"),
        "the daemon is left alone: {:?}",
        host.calls()
    );
    assert!(
        !host.ran("/bin/true"),
        "a running sandbox is not started again: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_selected_worktree_becomes_the_ssh_starting_directory() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let host = ready(FakeSbx::listing(&running), &project);

    let prepared = prepare_for_index(&fixture, &host, Some(0))
        .required_because("prepare the selected worktree")?;

    assert_eq!(
        prepared.working_directory,
        "/home/agent/work/example-repo/example-repo.tree-0"
    );
    assert_eq!(prepared.missing_worktree_index, None);
    Ok(())
}

#[test]
fn an_interactive_index_is_bounded_by_the_selected_projects_worktrees() -> Checked {
    let fixture = Fixture::new()?;
    let mut project = fixture.register("example-org/example-repo")?;
    project.metadata.provisioning.requested_worktrees = 5;
    metadata::update(&project.paths, &project.metadata)
        .required_because("record five managed worktrees")?;
    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let host = ready(FakeSbx::listing(&running), &project);
    let mut prompt = ScriptedPrompt::choosing_worktree(99);

    let prepared = prepare(
        &fixture.location,
        None,
        None,
        &host,
        &mut prompt,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .required_because("prepare the selected worktree")?;

    assert_eq!(
        prepared.working_directory, "/home/agent/work/example-repo/example-repo.tree-4",
        "the optimistic index the prompt accepted is brought down to what the metadata declares"
    );
    assert_eq!(
        prepared.clamped_worktree_index,
        Some(ClampedIndex {
            requested: MAX_WORKTREE_INDEX,
            opened: 4,
        }),
        "the difference between the confirmed value and the connection is not swallowed"
    );
    assert_eq!(prepared.missing_worktree_index, None);
    Ok(())
}

#[test]
fn an_interactive_index_inside_the_metadata_is_opened_without_a_warning() -> Checked {
    let fixture = Fixture::new()?;
    let mut project = fixture.register("example-org/example-repo")?;
    project.metadata.provisioning.requested_worktrees = 5;
    metadata::update(&project.paths, &project.metadata)
        .required_because("record five managed worktrees")?;
    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let host = ready(FakeSbx::listing(&running), &project);

    let prepared = prepare(
        &fixture.location,
        None,
        None,
        &host,
        &mut ScriptedPrompt::choosing_worktree(2),
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .required_because("prepare the selected worktree")?;

    assert_eq!(
        prepared.working_directory,
        "/home/agent/work/example-repo/example-repo.tree-2"
    );
    assert_eq!(
        prepared.clamped_worktree_index, None,
        "a value the project can satisfy is not reported as an adjustment"
    );
    Ok(())
}

#[test]
fn an_unconfigured_worktree_index_falls_back_to_the_repository_root() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let host = ready(FakeSbx::listing(&running), &project);

    let prepared = prepare_for_index(&fixture, &host, Some(1))
        .required_because("an unknown worktree falls back to the root")?;
    assert_eq!(prepared.working_directory, "/home/agent/work/example-repo");
    assert_eq!(prepared.missing_worktree_index, Some(1));
    Ok(())
}

#[test]
fn the_host_agent_has_to_be_out_of_reach_before_the_terminal_is_handed_over() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let running = format!("[{}]", fixture.entry(&project, "running")?);
    // daemonがSSH Agentを渡す状態で起動していた場合、中から到達できる。
    let host = ready(FakeSbx::listing(&running), &project).answering(
        &format!("exec {} -- ssh-add -L", project.sandbox),
        0,
        "ssh-rsa AAAA...\n",
    );

    let error = prepare_for(&fixture, &host).refused_because("an exposed agent stops the run")?;
    assert_eq!(error.first_id(), Some(ErrorId::SshAgentExposed));
    assert!(
        !host.ran(".sbx"),
        "the terminal is not handed over: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_stopped_project_is_started_without_a_terminal_and_waited_for() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let stopped = format!("[{}]", fixture.entry(&project, "stopped")?);
    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let host = ready(FakeSbx::listings(&[&stopped, &running]), &project);

    prepare_for(&fixture, &host).required_because("prepare")?;

    assert!(host.ran("/bin/true"), "{:?}", host.calls());
    assert_lifecycle(&host, "/bin/true")?;
    Ok(())
}

#[test]
fn a_project_without_a_sandbox_is_sent_back_to_add() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("Example-Org/Example-Repo")?;
    let host = ready(FakeSbx::listing("[]"), &project);

    let error = prepare_for(&fixture, &host).refused_because("open never creates a sandbox")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotCreated));
    let diagnostic = &error.diagnostics()[0];
    assert_eq!(diagnostic.description.id, "error-sandbox-not-created");
    assert_eq!(
        diagnostic.description.args,
        vec![
            ("project", "Example-Org/Example-Repo".to_string()),
            ("sandbox", project.sandbox.to_string())
        ]
    );
    let remediation = diagnostic
        .remediation
        .as_ref()
        .required_because("the user is told how to build the sandbox")?;
    assert_eq!(
        remediation.explanation.first().map(|message| message.id),
        Some("remediation-sandbox-not-created")
    );
    // 実行を求めるcommandは説明文へ埋め込まず、独立した一行として持つ。
    assert_eq!(
        remediation
            .commands
            .iter()
            .map(crate::design::text::CommandLine::as_str)
            .collect::<Vec<_>>(),
        vec!["sbxm prepare Example-Org/Example-Repo"]
    );
    assert!(!host.ran("daemon stop"), "the daemon is left alone");
    Ok(())
}

#[test]
fn an_unmanaged_project_is_refused_before_the_host_is_touched() -> Checked {
    let fixture = Fixture::new()?;
    let host = FakeSbx::listing("[]");

    let error = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        None,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .refused_because("a project that is not managed has nothing to open")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));
    assert!(
        host.calls().is_empty(),
        "the host is not asked anything before the target is decided: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_rebuild_in_progress_stops_the_connection() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: "2".repeat(64),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&project.paths, &metadata).required_because("record the intent")?;

    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let host = ready(FakeSbx::listing(&running), &project);

    let error =
        prepare_for(&fixture, &host).refused_because("a half-switched sandbox is not opened")?;
    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
    assert!(!host.ran("ls --json"), "nothing is asked of the runtime");
    Ok(())
}

#[test]
fn an_intent_recorded_after_the_selection_is_still_seen() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let host = ready(FakeSbx::listing(&running), &project);

    // 選択したあとにrebuildが始まった状態を、lock取得後のmetadataから読み直す。
    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: "2".repeat(64),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&project.paths, &metadata).required_because("record the intent")?;

    let error = prepare(
        &fixture.location,
        Some(&ProjectId::parse("example-org/example-repo").required()?),
        None,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .refused_because("the intent on disk decides, not the copy from the selection")?;
    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
    Ok(())
}

#[test]
fn a_sandbox_that_never_reaches_running_is_reported_rather_than_assumed() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let stopped = format!("[{}]", fixture.entry(&project, "stopped")?);
    let host = ready(FakeSbx::listing(&stopped), &project);

    let error = prepare_for(&fixture, &host)
        .refused_because("a sandbox that stays stopped is not connected")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotRunning));
    let remediation = error.diagnostics()[0]
        .remediation
        .as_ref()
        .required_because("the user is told how to look at it")?;
    assert!(
        remediation
            .commands
            .iter()
            .any(|command| command.as_str() == "sbxm status example-org/example-repo"),
        "the remediation names a command that can be run: {remediation:?}"
    );
    Ok(())
}

#[test]
fn a_missing_managed_worktree_stops_before_the_terminal_is_handed_over() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    // directoryはあってもGitのworktreeでなければ、宣言を満たしていない。
    let host = ready(FakeSbx::listing(&running), &project).answering(
        &format!(
            "exec {} -- git --git-dir {} worktree list --porcelain -z",
            project.sandbox,
            layout.bare_git_dir()
        ),
        0,
        &format!("worktree {}\0bare\0\0", layout.bare_root()),
    );

    let error = prepare_for(&fixture, &host).refused_because("a declared worktree has to exist")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxRepositoryUnusable));
    Ok(())
}

#[test]
fn an_engine_that_does_not_answer_stops_before_anything_is_read() -> Checked {
    let fixture = Fixture::new()?;
    fixture.register("example-org/example-repo")?;
    let host = FakeSbx::listing("[]").answering("version --format {{.Server.Version}}", 1, "");

    let error = prepare_for(&fixture, &host)
        .refused_because("without the engine there is nothing to open")?;
    assert_eq!(error.first_id(), Some(ErrorId::DockerUnreachable));
    assert!(!host.ran("ls --json"));
    Ok(())
}

#[test]
fn the_connection_hands_the_terminal_to_ssh() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let host = ready(FakeSbx::listing(&running), &project);
    let prepared = prepare_for(&fixture, &host).required_because("prepare")?;

    connect(&host, prepared, &mut RecordedOutput::new()).required_because("connect")?;
    let ssh = host.spec(&format!("{}.sbx", project.sandbox))?;
    assert_eq!(ssh.program, "ssh");
    assert_eq!(
        ssh.args,
        vec![
            "-t".to_string(),
            format!("{}.sbx", project.sandbox),
            "cd '/home/agent/work/example-repo/example-repo.tree-0' && exec \"${SHELL:-/bin/sh}\" -l".to_string(),
        ]
    );
    assert_eq!(
        ssh.output(),
        OutputPolicy::HandOver,
        "the terminal itself is handed over"
    );
    assert_eq!(
        ssh.timeout,
        TimeoutClass::Interactive,
        "the user decides when an interactive session ends"
    );
    Ok(())
}

#[test]
fn the_session_lease_is_released_before_a_connection_error_is_reported() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let host = ready(FakeSbx::listing(&running), &project).answering(
        &format!(
            "-t {}.sbx cd '/home/agent/work/example-repo/example-repo.tree-0' && exec \"${{SHELL:-/bin/sh}}\" -l",
            project.sandbox
        ),
        3,
        "",
    );
    let prepared = prepare_for(&fixture, &host).required_because("prepare")?;

    connect(&host, prepared, &mut RecordedOutput::new())
        .refused_because("a failed SSH child is reported")?;
    paths::acquire_exclusive_lock(
        &project.paths.session_lease_file(),
        Duration::from_millis(50),
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("the session lease is released before the error report")?;
    Ok(())
}

#[test]
fn the_project_lock_is_released_before_the_terminal_is_handed_over() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let host = ready(FakeSbx::listing(&running), &project);

    prepare_for(&fixture, &host).required_because("prepare")?;
    // 接続中に、別terminalの`stop`がこの案件を待たされない。
    paths::acquire_exclusive_lock(
        &project.paths.lock_file(),
        Duration::from_millis(50),
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("the lock covers the mutation, not the session")?;
    Ok(())
}

#[test]
fn the_session_lease_stays_held_until_the_terminal_session_ends() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let host = ready(FakeSbx::listing(&running), &project);

    let prepared = prepare_for(&fixture, &host).required_because("prepare")?;

    // project lockは外れても、SSH sessionの生存中はshared session leaseを持ち続ける。
    // 通常rebuild/destroyが取るexclusive leaseはここで拒否される。
    paths::acquire_exclusive_lock(
        &project.paths.session_lease_file(),
        Duration::from_millis(50),
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .refused_because("an active sbxm open session blocks a new exclusive session lease")?;

    // sessionが終わる（`Prepared`が破棄される）と、exclusive leaseを取得できる。
    drop(prepared);
    paths::acquire_exclusive_lock(
        &project.paths.session_lease_file(),
        Duration::from_millis(50),
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("the session lease releases once the session ends")?;
    Ok(())
}
