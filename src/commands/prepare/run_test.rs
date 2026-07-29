use super::super::world::{World, bench};
use super::*;
use crate::command::{OutputPolicy, TimeoutClass};
use crate::compatibility::SandboxState;
use crate::error::ErrorId;
use crate::hash::sha256_hex;
use crate::support::files::Placement;
use crate::testing::add_request::request;
use crate::testing::project::project_id;
use crate::testing::value::COMMIT;
use std::fs;

#[test]
fn a_project_that_is_not_registered_is_sent_to_add() {
    let bench = bench();
    let world = World::new();

    let error = run(
        &bench.config,
        &project_id("example-org/example-repo"),
        &world,
        bench.workspace_root.path(),
    )
    .expect_err("there is nothing to build yet");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));

    let diagnostic = &error.diagnostics()[0];
    assert_eq!(
        diagnostic.remediation.as_ref().map(|message| message.id),
        Some("remediation-project-not-managed")
    );
    assert!(
        world.invocations().is_empty(),
        "nothing is asked of the host: {:?}",
        world.invocations()
    );
}

#[test]
fn an_unregistered_project_gets_no_lock_file() {
    let bench = bench();
    let world = World::new();
    let project = project_id("example-org/example-repo");
    let paths = ProjectPaths::derive(&bench.config.base_path, &project.canonical());
    // lock fileを置ける状態、つまりmetadataのない`.sbxm`だけがある状態で確かめる。
    fs::create_dir_all(paths.sbxm_dir()).expect("the project directory is left behind");

    run(&bench.config, &project, &world, bench.workspace_root.path())
        .expect_err("there is nothing to build yet");

    assert!(
        !paths.lock_file().exists(),
        "an unregistered project is not given a lock file"
    );
    assert_eq!(
        fs::read_dir(paths.sbxm_dir())
            .expect("read the project directory")
            .count(),
        0,
        "nothing is written under an unregistered project"
    );
}

#[test]
fn a_rebuild_in_progress_builds_nothing() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None);
    crate::commands::add::run::run(&bench.config, &request, &world)
        .expect("the project is registered");

    let paths = ProjectPaths::derive(&bench.config.base_path, &request.project.canonical());
    let mut stored = metadata::load(&paths)
        .expect("read the metadata")
        .expect("present");
    stored.rebuild = Some(metadata::RebuildIntent {
        target_dockerfile_sha256: sha256_hex(b"target"),
        previous_dockerfile_sha256: stored.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&paths, &stored).expect("record the intent");

    let mark = world.mark();
    let error = run(
        &bench.config,
        &request.project,
        &world,
        bench.workspace_root.path(),
    )
    .expect_err("a half-switched project is not built on");
    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));

    let remediation = error.diagnostics()[0]
        .remediation
        .as_ref()
        .expect("the user is told how to get out of it");
    assert_eq!(remediation.id, "remediation-run-rebuild");
    let command = remediation
        .args
        .iter()
        .find(|(name, _)| *name == "command")
        .map(|(_, value)| value.clone())
        .expect("the remediation carries the command to run");
    assert_eq!(command, "sbxm rebuild Example-Org/Example-Repo");

    assert!(
        world.since(mark).is_empty(),
        "nothing is asked of the host: {:?}",
        world.since(mark)
    );
}

/// `add`と`prepare`が外部工程を呼ぶ順に並べた、失敗させる工程とその診断。
const STEPS: [(&str, ErrorId); 11] = [
    ("git clone git@github.com", ErrorId::ExternalCommandFailed),
    ("docker build", ErrorId::ExternalCommandFailed),
    ("docker image save", ErrorId::ExternalCommandFailed),
    ("sbx template load", ErrorId::ExternalCommandFailed),
    ("sbx create", ErrorId::ExternalCommandFailed),
    ("sbx cp --follow-link", ErrorId::ExternalCommandFailed),
    ("config --global user.name", ErrorId::ExternalCommandFailed),
    ("sbx secret ls", ErrorId::ExternalCommandFailed),
    ("git init --bare", ErrorId::ExternalCommandFailed),
    ("check-ref-format", ErrorId::InvalidBranchName),
    ("worktree add", ErrorId::ExternalCommandFailed),
];

#[test]
fn an_interruption_at_any_step_is_continued_by_the_same_prepare() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None);

    // 1工程ずつ後ろへずらして失敗させる。次の実行がそこまで進めることが継続の証拠になる。
    for (step, expected) in STEPS {
        world.failing(step);
        let error = bench
            .build(&world, &request)
            .expect_err("the run stops at the step that failed");
        assert_eq!(error.first_id(), Some(expected), "{step}");
        world.nothing_fails();
    }

    // 最後に失敗したのはworktree作成であり、続きの実行はそこから進む。
    let mark = world.mark();
    let output = bench
        .build(&world, &request)
        .expect("the same add finishes");
    let tail = world.since(mark);

    assert!(!output.already_built);
    assert_eq!(output.mode, CreationMode::Attached);
    assert_eq!(output.start_ref, "main");
    assert_eq!(output.sandbox_state, SandboxState::Running);
    assert_eq!(output.worktrees.len(), 1);
    assert_eq!(output.worktrees[0].path, "example-repo.tree-0");
    assert_eq!(output.worktrees[0].head.as_deref(), Some(COMMIT));
    assert_eq!(output.files.len(), 1);
    assert_eq!(
        output.files[0].placement,
        Placement::Unchanged,
        "an earlier run placed the file, and an identical destination is left alone"
    );

    let stored = bench.stored("Example-Org/Example-Repo");
    assert_eq!(stored.provisioning.start_ref.as_deref(), Some("main"));

    // 成功済みの成果物は作り直さない。
    for done in [
        "git clone git@github.com",
        "docker build",
        "sbx template load",
        "sbx create",
        "git init --bare",
    ] {
        assert!(
            !tail.iter().any(|call| call.contains(done)),
            "{done} was already done: {tail:?}"
        );
    }
    assert_eq!(
        tail.iter()
            .filter(|call| call.contains("worktree add"))
            .count(),
        1,
        "the run continues with the step that had failed: {tail:?}"
    );
    // archiveは工程へ到達するたびに作り直す。
    assert!(tail.iter().any(|call| call.contains("docker image save")));
}

#[test]
fn a_finished_build_is_a_no_op_for_the_same_add() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None);
    bench.build(&world, &request).expect("the first run builds");

    let mark = world.mark();
    let output = bench
        .build(&world, &request)
        .expect("the second run changes nothing");

    assert!(output.already_built);
    for forbidden in [
        "docker build",
        "docker image save",
        "sbx template load",
        "sbx create",
        "sbx daemon stop",
        "sbx cp",
        "worktree add",
        "git clone",
    ] {
        assert!(
            !world
                .since(mark)
                .iter()
                .any(|call| call.contains(forbidden)),
            "a finished project must not run {forbidden}: {:?}",
            world.since(mark)
        );
    }
}

#[test]
fn a_head_that_cannot_be_read_is_left_unknown() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None);
    bench.build(&world, &request).expect("the first run builds");

    // 停止中のSandboxと同じく、worktreeのHEADだけが読めない状態にする。読めない読み取りも
    // 出力は返すので、成功したかどうかはexit statusでしか分からない。
    world.failing_with("rev-parse HEAD", "fatal: not a git repository\n");
    let output = run(
        &bench.config,
        &request.project,
        &world,
        bench.workspace_root.path(),
    )
    .expect("a project that is built stays built");

    assert!(output.already_built);
    assert_eq!(output.worktrees.len(), 1);
    assert_eq!(
        output.worktrees[0].head, None,
        "the output of a failed read is not reported as a HEAD"
    );
    assert_eq!(
        output.worktrees[0].created_from, "refs/remotes/origin/main",
        "what metadata declares is still reported"
    );
    assert_eq!(output.worktrees[0].mode, CreationMode::Attached);
}

#[test]
fn a_head_that_reads_back_empty_is_left_unknown() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None);
    bench.build(&world, &request).expect("the first run builds");

    // 成功しながら何も答えない読み取り。値がない以上、観測できたことにはならない。
    world.succeeding_silently("rev-parse HEAD");
    let output = run(
        &bench.config,
        &request.project,
        &world,
        bench.workspace_root.path(),
    )
    .expect("a project that is built stays built");

    assert!(output.already_built);
    assert_eq!(output.worktrees.len(), 1);
    assert_eq!(
        output.worktrees[0].head, None,
        "an empty answer is not reported as a HEAD"
    );
}

/// 編集後のDockerfileの内容。世代が変わったことだけが要る。
const EDITED_DOCKERFILE: &[u8] = b"FROM example:edited\n";

#[test]
fn a_dockerfile_edited_after_the_image_exists_finishes_on_the_generation_it_started_from() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None);

    // imageまで組み上がり、Sandboxの作成で中断した実行を作る。
    world.failing("sbx create");
    bench
        .build(&world, &request)
        .expect_err("the run stops at sandbox creation");
    world.nothing_fails();

    let started_from = bench
        .stored("Example-Org/Example-Repo")
        .provisioning
        .dockerfile_sha256;
    let paths = ProjectPaths::derive(&bench.config.base_path, &request.project.canonical());
    fs::write(paths.dockerfile(), EDITED_DOCKERFILE).expect("edit the Dockerfile");

    let mark = world.mark();
    let output = run(
        &bench.config,
        &request.project,
        &world,
        bench.workspace_root.path(),
    )
    .expect("the interrupted run finishes");

    assert_eq!(
        output
            .warnings
            .iter()
            .map(|message| message.id)
            .collect::<Vec<_>>(),
        vec!["warning-dockerfile-changed-during-build"]
    );
    assert_eq!(
        bench
            .stored("Example-Org/Example-Repo")
            .provisioning
            .dockerfile_sha256,
        started_from,
        "the generation the build started from is the one it is finished on"
    );
    let edited = image::image_name(
        &SandboxName::derive(&request.project.canonical()),
        &sha256_hex(EDITED_DOCKERFILE),
    );
    assert!(
        !world.since(mark).iter().any(|call| call.contains(&edited)),
        "the edited Dockerfile is left for rebuild: {:?}",
        world.since(mark)
    );
}

#[test]
fn a_dockerfile_edited_before_any_image_exists_is_the_generation_that_gets_built() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None);
    crate::commands::add::run::run(&bench.config, &request, &world)
        .expect("the project is registered");

    let paths = ProjectPaths::derive(&bench.config.base_path, &request.project.canonical());
    fs::write(paths.dockerfile(), EDITED_DOCKERFILE).expect("edit the Dockerfile");
    let edited = sha256_hex(EDITED_DOCKERFILE);

    let output = run(
        &bench.config,
        &request.project,
        &world,
        bench.workspace_root.path(),
    )
    .expect("the build runs on the Dockerfile that is there");

    assert!(output.warnings.is_empty(), "{:?}", output.warnings);
    assert_eq!(
        bench
            .stored("Example-Org/Example-Repo")
            .provisioning
            .dockerfile_sha256,
        edited,
        "the edited Dockerfile becomes the generation to build"
    );
    assert!(
        world.ran(&image::image_name(
            &SandboxName::derive(&request.project.canonical()),
            &edited
        )),
        "{:?}",
        world.invocations()
    );
}

#[test]
fn git_is_given_the_placeholder_before_it_reaches_github() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None);
    bench.build(&world, &request).expect("the build completes");

    let calls = world.invocations();
    let position = |needle: &str| {
        calls
            .iter()
            .position(|call| call.contains(needle))
            .unwrap_or_else(|| panic!("no command matched {needle}: {calls:?}"))
    };
    assert!(
        position("credential.https://github.com.helper") < position("fetch --prune origin"),
        "a fetch without the credential asks for a username and never finishes"
    );
}

#[test]
fn a_missing_secret_stops_the_build_and_the_same_add_continues_once_it_is_there() {
    let bench = bench();
    let world = World::new();
    world.secrets.borrow_mut().clear();
    let request = request("Example-Org/Example-Repo", None, None);

    let error = bench
        .build(&world, &request)
        .expect_err("a build without repository access cannot continue");
    assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
    assert!(
        !world.ran("git init --bare"),
        "the sandbox repository is not made without the secret"
    );
    // custom secretはSandboxの作成時に結び付く。先に作ってしまうと、登録しても
    // placeholderの届かないSandboxが残り、作り直しを強いることになる。
    assert!(
        !world.ran("sbx create"),
        "the sandbox is not created before the secret it has to be built with"
    );
    assert!(
        !world.ran("docker build"),
        "the image is not built before the missing secret is reported"
    );

    for host in crate::support::secret::GITHUB_HOSTS {
        world.secrets.borrow_mut().push(host.to_string());
    }
    let output = bench
        .build(&world, &request)
        .expect("the same add continues once the secret is registered");
    assert_eq!(output.worktrees.len(), 1);
    assert_eq!(
        world
            .invocations()
            .iter()
            .filter(|call| call.contains("sbx create"))
            .count(),
        1,
        "the sandbox that was already there is reused"
    );
}

#[test]
fn three_detached_worktrees_start_from_one_commit_of_the_named_branch() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", Some(3), Some("develop"));

    let output = bench.build(&world, &request).expect("build");
    assert_eq!(output.mode, CreationMode::Detached);
    assert_eq!(output.start_ref, "develop");
    assert_eq!(output.worktrees.len(), 3);
    for (index, worktree) in output.worktrees.iter().enumerate() {
        assert_eq!(worktree.path, format!("example-repo.tree-{index}"));
        assert_eq!(worktree.created_from, "refs/remotes/origin/develop");
        assert_eq!(worktree.head.as_deref(), Some(COMMIT));
        assert!(
                world.ran(&format!(
                    "worktree add --detach /home/agent/work/example-repo/example-repo.tree-{index} refs/remotes/origin/develop"
                )),
                "{:?}",
                world.invocations()
            );
    }
    // bare repositoryとworktreeは、1 treeでも3 treesでも分かれている。
    assert!(world.ran("git init --bare /home/agent/work/example-repo/.git"));
    assert!(world.ran("remote add origin https://github.com/Example-Org/Example-Repo.git"));
}

/// managed worktreeが持ち込んだ`mise.toml`のpath。
const DECLARED_MISE: &str = "/home/agent/work/example-repo/example-repo.tree-0/mise.toml";

#[test]
fn what_the_tools_answer_reaches_the_output() {
    let bench = bench();
    let world = World::new();
    world.carrying(DECLARED_MISE);

    let output = bench
        .build(&world, &request("Example-Org/Example-Repo", None, None))
        .expect("build");
    assert_eq!(output.notes.len(), 1, "{:?}", output.notes);
    assert_eq!(output.notes[0].items, vec![DECLARED_MISE.to_string()]);
}

#[test]
fn a_tool_the_sandbox_lacks_never_reaches_the_output() {
    let bench = bench();
    let world = World::new();
    world.without("mise");
    world.carrying(DECLARED_MISE);

    let output = bench
        .build(&world, &request("Example-Org/Example-Repo", None, None))
        .expect("build");
    assert!(output.notes.is_empty(), "{:?}", output.notes);
    assert!(
        !world.ran("mise.toml"),
        "the declared files are not even looked for: {:?}",
        world.invocations()
    );
}

#[test]
fn a_sandbox_without_gh_is_never_asked_to_configure_it() {
    let bench = bench();
    let world = World::new();
    world.without("gh");

    bench
        .build(&world, &request("Example-Org/Example-Repo", None, None))
        .expect("build");
    assert!(!world.ran("git_protocol"), "{:?}", world.invocations());
}

#[test]
fn the_long_steps_forward_their_progress_and_the_read_steps_are_captured() {
    let bench = bench();
    let world = World::new();
    bench
        .build(&world, &request("Example-Org/Example-Repo", None, None))
        .expect("build");

    for (needle, timeout) in [
        ("docker build", TimeoutClass::ImageBuild),
        ("docker image save", TimeoutClass::ImageBuild),
        ("git clone git@github.com", TimeoutClass::RepositoryTransfer),
        ("sbx template load", TimeoutClass::SandboxLifecycle),
        ("sbx create", TimeoutClass::SandboxLifecycle),
        ("fetch --prune origin", TimeoutClass::RepositoryTransfer),
    ] {
        assert_eq!(
            world.policy_of(needle),
            Some((OutputPolicy::Passthrough, timeout)),
            "{needle} shows its progress while it runs"
        );
    }

    for needle in [
        "sbx ls --json",
        "docker image inspect",
        "sbx secret ls",
        "sbx template ls --json",
    ] {
        assert_eq!(
            world.policy_of(needle).map(|(output, _)| output),
            Some(OutputPolicy::Capture),
            "{needle} is read rather than shown"
        );
    }
}

#[test]
fn the_declared_file_is_placed_once_and_left_alone_afterwards() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None);
    bench.build(&world, &request).expect("build");

    assert_eq!(
        world
            .digests
            .borrow()
            .get("/home/agent/.config/example/config.toml")
            .map(String::as_str),
        Some(sha256_hex(b"declared = true\n").as_str()),
        "the declared file reaches the destination it was declared for"
    );
    assert!(
        !world.present.borrow().contains("/tmp/sbxm-file-0"),
        "the staged copy does not survive the placement"
    );

    // 同じ内容の再配置は、Sandboxへ書き込まない。
    let world = World::new();
    world.digests.borrow_mut().insert(
        "/home/agent/.config/example/config.toml".to_string(),
        sha256_hex(b"declared = true\n"),
    );
    world
        .present
        .borrow_mut()
        .insert("/home/agent/.config/example/config.toml".to_string());
    let output = bench.build(&world, &request).expect("build");
    assert_eq!(output.files[0].placement, Placement::Unchanged);
    assert!(
        !world.ran("sbx cp"),
        "an identical destination is left alone"
    );
}
