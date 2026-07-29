use super::*;
use crate::command::{EnvPolicy, OutputPolicy, TimeoutClass};
use crate::metadata::{self, RebuildIntent};
use crate::paths::{self, PRIVATE_FILE_MODE, PathScope};
use crate::testing::host::{FakeSbx, isolated_agent};
use crate::testing::poll::poll;
use crate::testing::project::{Fixture, Registered, fixture, project_id};
use crate::testing::prompt::ScriptedPrompt;
use std::time::Duration;

/// Docker疎通とworktree一覧に応答するhost。
fn ready(host: FakeSbx, project: &Registered) -> FakeSbx {
    let layout = SandboxLayout::new(&project.metadata.canonical_id);
    let listing = format!(
        "worktree {}\0bare\0\0worktree {}/{}\0branch refs/heads/main\0\0",
        layout.bare_root(),
        layout.bare_root(),
        layout.worktree_name(0)
    );
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
        &fixture.config,
        None,
        host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
    )
}

#[test]
fn a_running_project_is_opened_without_touching_the_daemon() {
    let fixture = fixture();
    let project = fixture.register("Example-Org/Example-Repo");
    let running = format!("[{}]", fixture.entry(&project, "running"));
    let host = ready(FakeSbx::listing(&running), &project);

    let prepared = prepare_for(&fixture, &host).expect("prepare");

    assert_eq!(prepared.ssh_host, format!("{}.sbx", project.sandbox));
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
}

#[test]
fn the_host_agent_has_to_be_out_of_reach_before_the_terminal_is_handed_over() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let running = format!("[{}]", fixture.entry(&project, "running"));
    // daemonがSSH Agentを渡す状態で起動していた場合、中から到達できる。
    let host = ready(FakeSbx::listing(&running), &project).answering(
        &format!("exec {} -- ssh-add -L", project.sandbox),
        0,
        "ssh-rsa AAAA...\n",
    );

    let error = prepare_for(&fixture, &host).expect_err("an exposed agent stops the run");
    assert_eq!(error.first_id(), Some(ErrorId::SshAgentExposed));
    assert!(
        !host.ran(".sbx"),
        "the terminal is not handed over: {:?}",
        host.calls()
    );
}

#[test]
fn a_stopped_project_is_started_without_a_terminal_and_waited_for() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let stopped = format!("[{}]", fixture.entry(&project, "stopped"));
    let running = format!("[{}]", fixture.entry(&project, "running"));
    let host = ready(FakeSbx::listings(&[&stopped, &running]), &project);

    prepare_for(&fixture, &host).expect("prepare");

    assert!(host.ran("/bin/true"), "{:?}", host.calls());
    let start = host.spec("/bin/true");
    assert_eq!(
        start.output,
        OutputPolicy::Passthrough,
        "the runtime's own progress is shown as it is"
    );
    assert_eq!(start.env, EnvPolicy::InheritWithoutSshAgent);
}

#[test]
fn a_project_without_a_sandbox_is_sent_back_to_add() {
    let fixture = fixture();
    let project = fixture.register("Example-Org/Example-Repo");
    let host = ready(FakeSbx::listing("[]"), &project);

    let error = prepare_for(&fixture, &host).expect_err("open never creates a sandbox");
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
        .expect("the user is told how to build the sandbox");
    assert_eq!(remediation.id, "remediation-sandbox-not-created");
    assert_eq!(
        remediation.args,
        vec![("command", "sbxm add Example-Org/Example-Repo".to_string())]
    );
    assert!(!host.ran("daemon stop"), "the daemon is left alone");
}

#[test]
fn an_unmanaged_project_is_refused_before_the_host_is_touched() {
    let fixture = fixture();
    let host = FakeSbx::listing("[]");

    let error = prepare(
        &fixture.config,
        Some(&project_id("example-org/example-repo")),
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
    )
    .expect_err("a project that is not managed has nothing to open");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));
    assert!(
        host.calls().is_empty(),
        "the host is not asked anything before the target is decided: {:?}",
        host.calls()
    );
}

#[test]
fn a_rebuild_in_progress_stops_the_connection() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: "2".repeat(64),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&project.paths, &metadata).expect("record the intent");

    let running = format!("[{}]", fixture.entry(&project, "running"));
    let host = ready(FakeSbx::listing(&running), &project);

    let error = prepare_for(&fixture, &host).expect_err("a half-switched sandbox is not opened");
    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
    assert!(!host.ran("ls --json"), "nothing is asked of the runtime");
}

#[test]
fn an_intent_recorded_after_the_selection_is_still_seen() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let running = format!("[{}]", fixture.entry(&project, "running"));
    let host = ready(FakeSbx::listing(&running), &project);

    // 選択したあとにrebuildが始まった状態を、lock取得後のmetadataから読み直す。
    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: "2".repeat(64),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&project.paths, &metadata).expect("record the intent");

    let error = prepare(
        &fixture.config,
        Some(&ProjectId::parse("example-org/example-repo").unwrap()),
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
        poll(),
    )
    .expect_err("the intent on disk decides, not the copy from the selection");
    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));
}

#[test]
fn a_sandbox_that_never_reaches_running_is_reported_rather_than_assumed() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let stopped = format!("[{}]", fixture.entry(&project, "stopped"));
    let host = ready(FakeSbx::listing(&stopped), &project);

    let error =
        prepare_for(&fixture, &host).expect_err("a sandbox that stays stopped is not connected");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotRunning));
    let remediation = error.diagnostics()[0]
        .remediation
        .as_ref()
        .expect("the user is told how to look at it");
    assert!(
        remediation
            .args
            .iter()
            .any(|(_, value)| value == "sbxm status example-org/example-repo"),
        "the remediation names a command that can be run: {remediation:?}"
    );
}

#[test]
fn a_missing_managed_worktree_stops_before_the_terminal_is_handed_over() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let running = format!("[{}]", fixture.entry(&project, "running"));
    let layout = SandboxLayout::new(&project.metadata.canonical_id);
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

    let error = prepare_for(&fixture, &host).expect_err("a declared worktree has to exist");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxRepositoryUnusable));
}

#[test]
fn an_engine_that_does_not_answer_stops_before_anything_is_read() {
    let fixture = fixture();
    fixture.register("example-org/example-repo");
    let host = FakeSbx::listing("[]").answering("version --format {{.Server.Version}}", 1, "");

    let error =
        prepare_for(&fixture, &host).expect_err("without the engine there is nothing to open");
    assert_eq!(error.first_id(), Some(ErrorId::DockerUnreachable));
    assert!(!host.ran("ls --json"));
}

#[test]
fn the_connection_hands_the_terminal_to_ssh() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let running = format!("[{}]", fixture.entry(&project, "running"));
    let host = ready(FakeSbx::listing(&running), &project);
    let prepared = prepare_for(&fixture, &host).expect("prepare");

    connect(&host, &prepared).expect("connect");
    let ssh = host.spec(&format!("{}.sbx", project.sandbox));
    assert_eq!(ssh.program, "ssh");
    assert_eq!(ssh.args, vec![format!("{}.sbx", project.sandbox)]);
    assert_eq!(
        ssh.output,
        OutputPolicy::Inherit,
        "the terminal itself is handed over"
    );
    assert_eq!(
        ssh.timeout,
        TimeoutClass::Interactive,
        "the user decides when an interactive session ends"
    );
}

#[test]
fn the_project_lock_is_released_before_the_terminal_is_handed_over() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    let running = format!("[{}]", fixture.entry(&project, "running"));
    let host = ready(FakeSbx::listing(&running), &project);

    prepare_for(&fixture, &host).expect("prepare");
    // 接続中に、別terminalの`stop`がこの案件を待たされない。
    paths::acquire_exclusive_lock(
        &project.paths.lock_file(),
        Duration::from_millis(50),
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .expect("the lock covers the mutation, not the session");
}
