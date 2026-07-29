use super::*;
use crate::command::{EnvPolicy, OutputPolicy, TimeoutClass};
use crate::hash::sha256_hex;
use crate::testing::host::{FakeSbx, isolated_agent, registered_secret};
use crate::testing::image::template_listing;
use crate::testing::poll::poll;
use crate::testing::project::{fixture, project_id};
use crate::testing::protection::clean_host;
use crate::testing::value::COMMIT;
use std::os::unix::fs::PermissionsExt;

/// 再作成後の検証を通るSandbox。secretがあり、SSH Agentへ到達できない。
fn verified(host: FakeSbx, name: &str) -> FakeSbx {
    isolated_agent(registered_secret(host, name), name)
}

#[test]
fn a_dockerfile_that_did_not_change_is_a_no_op() {
    let fixture = fixture();
    let mut project = fixture.register("example-org/example-repo");
    // 適用済みhashと同じ内容のDockerfileを置く。
    std::fs::write(project.paths.dockerfile(), "unchanged\n").unwrap();
    project.metadata.provisioning.dockerfile_sha256 = sha256_hex(b"unchanged\n");
    metadata::update(&project.paths, &project.metadata).unwrap();

    let host = FakeSbx::listing(&format!("[{}]", fixture.entry(&project, "running")));
    let output = run(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
        poll(),
    )
    .expect("nothing to apply");

    assert!(output.unchanged);
    assert_eq!(output.applied, sha256_hex(b"unchanged\n"));
    assert!(
        !host.ran("build") && !host.ran("rm "),
        "a no-op touches nothing: {:?}",
        host.calls()
    );
}

#[test]
fn a_project_whose_build_never_finished_is_sent_to_add_even_with_the_same_dockerfile() {
    let fixture = fixture();
    let mut project = fixture.register("example-org/example-repo");
    // `add`は登録時に適用済みhashを書く。Sandboxを作る前に中断した案件は、
    // 現在のDockerfileと同じhashを持ったまま`not-created`で残る。
    std::fs::write(project.paths.dockerfile(), "unchanged\n").unwrap();
    project.metadata.provisioning.dockerfile_sha256 = sha256_hex(b"unchanged\n");
    metadata::update(&project.paths, &project.metadata).unwrap();

    let host = FakeSbx::listing("[]");
    let error = run(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
        poll(),
    )
    .expect_err("there is no sandbox to report as unchanged");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotCreated));
}

#[test]
fn a_project_that_is_not_managed_cannot_be_rebuilt() {
    let fixture = fixture();
    let host = FakeSbx::listing("[]");
    let error = run(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
        poll(),
    )
    .expect_err("there is nothing to rebuild");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));
}

#[test]
fn a_stopped_sandbox_is_started_rather_than_handed_back_to_the_user() {
    // `rebuild`はこのSandboxをこれから作り直す。保存状態を読むためだけの起動を
    // 利用者へ求めない。
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();
    let name = project.sandbox.as_str();

    let stopped = format!("[{}]", fixture.entry(&project, "stopped"));
    let running = format!("[{}]", fixture.entry(&project, "running"));
    let host = FakeSbx::listings(&[&stopped, &running]);

    // 起動の先で止まってよい。ここで見たいのは、停止を理由に拒否しないことである。
    let _ = run(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
        poll(),
    );

    assert!(
        host.ran(&format!("exec {name} -- /bin/true")),
        "the sandbox is started: {:?}",
        host.calls()
    );
}

#[test]
fn a_project_without_a_sandbox_is_refused_with_the_command_that_helps() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();

    let absent = FakeSbx::listing("[]");
    let error = run(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &absent,
        &fixture.workspace_root,
        poll(),
    )
    .expect_err("a project without a sandbox has nothing to switch");
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotCreated));
    assert!(!absent.ran("build"), "nothing is built");
}

#[test]
fn unsaved_work_stops_the_rebuild_before_anything_is_built() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();
    let layout = SandboxLayout::new(&project.metadata.canonical_id);
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project).answering(
        &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
        0,
        "? scratch.txt\0",
    );

    let error = run(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
        poll(),
    )
    .expect_err("a dirty worktree is not recreated");
    assert_eq!(error.first_id(), Some(ErrorId::UnsavedWork));
    assert!(
        !host.ran("build"),
        "the existing sandbox is untouched: {:?}",
        host.calls()
    );
}

#[test]
fn the_sandbox_to_switch_is_decided_after_the_new_generation_is_ready() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();
    let target = sha256_hex(b"FROM scratch\n");
    let image = image::image_name(&project.sandbox, &target);
    let workspace = fixture.workspace_root.join(project.sandbox.as_str());
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).unwrap();

    let host = clean_host(&fixture, &project)
            .answering(&format!("image ls --quiet {image}"), 0, "sha256:new\n")
            .answering(
                &format!("image inspect {image}"),
                0,
                &format!(
                    r#"[{{"Id":"sha256:new","Config":{{"Labels":{{"io.crescware.sbxm.canonical-id":"example-org/example-repo","io.crescware.sbxm.dockerfile-sha256":"{target}","io.crescware.sbxm.metadata-version":"1"}}}}}}]"#
                ),
            )
            .answering(
                "template ls --json",
                0,
                &template_listing(&image),
            );

    // 一覧は末尾から取り出される。世代の準備が終わるまでのあいだに、
    // 対象Sandboxが手作業で消された状況を作る。
    let running = format!("[{}]", fixture.entry(&project, "running"));
    let created = format!(
        r#"[{{"name":"{}","state":"running","workspace":"{}","template":"{image}","active_sessions":0}}]"#,
        project.sandbox,
        workspace.display()
    );
    // 一覧は末尾から取り出される。世代の準備が終わったあとの観測で、対象Sandboxが
    // 手作業で消えている状況になる。
    *host.listing.borrow_mut() = vec![created, "[]".to_string(), "[]".to_string(), running];

    let layout = SandboxLayout::new(&project.metadata.canonical_id);
    let git_dir = layout.bare_git_dir();
    let worktree = layout.worktree(0);
    let name = project.sandbox.as_str();
    let host = verified(host, name)
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} rev-parse --is-bare-repository"),
            0,
            "true\n",
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} config --get-all remote.origin.url"),
            0,
            "https://github.com/example-org/example-repo.git\n",
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} config --get-all remote.origin.fetch"),
            0,
            "+refs/heads/*:refs/remotes/origin/*\n",
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} rev-parse refs/remotes/origin/main"),
            0,
            &format!("{COMMIT}\n"),
        )
        .answering(
            &format!("exec {name} -- git -C {worktree} rev-parse HEAD"),
            0,
            &format!("{COMMIT}\n"),
        )
        .answering(
            &format!(
                "exec {name} -- git -C {worktree} rev-parse --path-format=absolute --git-common-dir"
            ),
            0,
            &format!("{git_dir}\n"),
        )
        .answering(
            &format!("exec {name} -- git -C {worktree} symbolic-ref -q HEAD"),
            0,
            "refs/heads/main\n",
        );

    run(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
        poll(),
    )
    .expect("the sandbox that is gone is created instead of removed");

    assert!(
        !host.ran("rm "),
        "a sandbox that no longer exists is not removed again: {:?}",
        host.calls()
    );
    assert!(
        host.ran("create --name"),
        "the run continued from the creation step: {:?}",
        host.calls()
    );
    // 外部toolの進捗は隠さず、SSH Agentを渡さず、lifecycleのtimeoutで実行する。
    let creation = host.spec("create --name");
    assert_eq!(creation.output, OutputPolicy::Passthrough);
    assert_eq!(creation.env, EnvPolicy::InheritWithoutSshAgent);
    assert_eq!(creation.timeout, TimeoutClass::SandboxLifecycle);
}

#[test]
fn a_new_generation_that_cannot_be_produced_leaves_the_existing_sandbox_alone() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();
    // buildは走るが、そのあともimageは一覧に現れない。
    let host = clean_host(&fixture, &project);

    let error = run(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
        poll(),
    )
    .expect_err("the new generation never became usable");
    assert_eq!(error.first_id(), Some(ErrorId::ImageUnusable));
    assert!(
        !host.ran("rm ") && !host.ran("create --name"),
        "the sandbox that is still running is untouched: {:?}",
        host.calls()
    );

    let stored = metadata::load(&project.paths).unwrap().expect("present");
    assert!(
        stored.rebuild.is_none(),
        "no generation was fixed, so there is nothing to continue"
    );
    assert_eq!(
        stored.provisioning.dockerfile_sha256, project.metadata.provisioning.dockerfile_sha256,
        "the applied generation did not move"
    );
}

#[test]
fn a_fixed_generation_with_neither_artifacts_nor_its_dockerfile_says_how_to_recover() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    // Dockerfileは、固定した世代とは別の内容へ変わっている。
    std::fs::write(project.paths.dockerfile(), "FROM alpine\n").unwrap();

    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: sha256_hex(b"FROM scratch\n"),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&project.paths, &metadata).unwrap();

    let host = clean_host(&fixture, &project);
    let error = run(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
        poll(),
    )
    .expect_err("generations are never mixed");
    assert_eq!(error.first_id(), Some(ErrorId::RebuildGenerationMissing));

    let diagnostic = &error.diagnostics()[0];
    assert_eq!(
        diagnostic.remediation.as_ref().map(|message| message.id),
        Some("remediation-rebuild-generation-missing")
    );
    assert!(
        !host.ran("build"),
        "the current Dockerfile is not built under the fixed generation's name: {:?}",
        host.calls()
    );
}

#[test]
fn a_stopped_previous_generation_is_started_so_its_saved_state_can_be_read() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();
    let target = sha256_hex(b"FROM scratch\n");
    let previous = project.metadata.provisioning.dockerfile_sha256.clone();

    // Sandboxを削除する前に中断した状態。
    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: target.clone(),
        previous_dockerfile_sha256: previous.clone(),
    });
    metadata::update(&project.paths, &metadata).unwrap();

    let image = image::image_name(&project.sandbox, &target);
    let workspace = fixture.workspace_root.join(project.sandbox.as_str());
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).unwrap();

    let stopped = format!("[{}]", fixture.entry(&project, "stopped"));
    let running = format!("[{}]", fixture.entry(&project, "running"));
    let created = format!(
        r#"[{{"name":"{}","state":"running","workspace":"{}","template":"{image}","active_sessions":0}}]"#,
        project.sandbox,
        workspace.display()
    );
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).unwrap();

    let host = clean_host(&fixture, &project)
            .answering(&format!("image ls --quiet {image}"), 0, "sha256:new\n")
            .answering(
                &format!("image inspect {image}"),
                0,
                &format!(
                    r#"[{{"Id":"sha256:new","Config":{{"Labels":{{"io.crescware.sbxm.canonical-id":"example-org/example-repo","io.crescware.sbxm.dockerfile-sha256":"{target}","io.crescware.sbxm.metadata-version":"1"}}}}}}]"#
                ),
            )
            .answering(
                "template ls --json",
                0,
                &template_listing(&image),
            );

    let layout = SandboxLayout::new(&project.metadata.canonical_id);
    let git_dir = layout.bare_git_dir();
    let worktree = layout.worktree(0);
    let name = project.sandbox.as_str();
    let host = verified(host, name)
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} rev-parse --is-bare-repository"),
            0,
            "true\n",
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} config --get-all remote.origin.url"),
            0,
            "https://github.com/example-org/example-repo.git\n",
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} config --get-all remote.origin.fetch"),
            0,
            "+refs/heads/*:refs/remotes/origin/*\n",
        )
        .answering(
            &format!("exec {name} -- git --git-dir {git_dir} rev-parse refs/remotes/origin/main"),
            0,
            &format!("{COMMIT}\n"),
        )
        .answering(
            &format!("exec {name} -- git -C {worktree} rev-parse HEAD"),
            0,
            &format!("{COMMIT}\n"),
        )
        .answering(
            &format!(
                "exec {name} -- git -C {worktree} rev-parse --path-format=absolute --git-common-dir"
            ),
            0,
            &format!("{git_dir}\n"),
        )
        .answering(
            &format!("exec {name} -- git -C {worktree} symbolic-ref -q HEAD"),
            0,
            "refs/heads/main\n",
        );
    // 一覧は末尾から取り出される。停止中のprevious世代を起動し、検査してから消す。
    *host.listing.borrow_mut() = vec![
        created,
        "[]".to_string(),
        "[]".to_string(),
        running.clone(),
        running,
        stopped.clone(),
        stopped,
    ];

    run(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
        poll(),
    )
    .expect("the fixed generation is completed from a stopped previous one");

    let calls = host.calls();
    let started = calls
        .iter()
        .position(|args| args.join(" ").contains("/bin/true"))
        .expect("the stopped sandbox is started before it is inspected");
    let removed = calls
        .iter()
        .position(|args| args.first().is_some_and(|arg| arg == "rm"))
        .expect("the previous generation is removed");
    assert!(
        started < removed,
        "the saved state is read from a running sandbox: {calls:?}"
    );
}

#[test]
fn an_interrupted_rebuild_continues_from_the_generation_it_fixed() {
    let fixture = fixture();
    let project = fixture.register("example-org/example-repo");
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").unwrap();
    let target = sha256_hex(b"FROM scratch\n");

    // Sandbox削除の直後で中断した状態を作る。
    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: target.clone(),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&project.paths, &metadata).unwrap();

    let image = image::image_name(&project.sandbox, &target);
    let workspace = fixture.workspace_root.join(project.sandbox.as_str());
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).unwrap();
    let created = format!(
        r#"[{{"name":"{}","state":"running","workspace":"{}","template":"{image}","active_sessions":0}}]"#,
        project.sandbox,
        workspace.display()
    );

    // 一覧は、run、Switch、作成前の確認、作成後の確認の順に読まれる。
    let host = FakeSbx::listings(&["[]", "[]", "[]", &created])
            // 固定した世代のimageは既にbuild済みである。
            .answering(&format!("image ls --quiet {image}"), 0, "sha256:new\n")
            .answering(
                &format!("image inspect {image}"),
                0,
                &format!(
                    r#"[{{"Id":"sha256:new","Config":{{"Labels":{{"io.crescware.sbxm.canonical-id":"example-org/example-repo","io.crescware.sbxm.dockerfile-sha256":"{target}","io.crescware.sbxm.metadata-version":"1"}}}}}}]"#
                ),
            )
            .answering(
                "template ls --json",
                0,
                &template_listing(&image),
            );
    // 再作成後のSandbox内で、共有repositoryとworktreeが期待どおりに揃う。
    let layout = SandboxLayout::new(&project.metadata.canonical_id);
    let git_dir = layout.bare_git_dir();
    let worktree = layout.worktree(0);
    let host = verified(host, project.sandbox.as_str())
        .answering(
            &format!(
                "exec {} -- git --git-dir {git_dir} rev-parse --is-bare-repository",
                project.sandbox
            ),
            0,
            "true\n",
        )
        .answering(
            &format!(
                "exec {} -- git --git-dir {git_dir} config --get-all remote.origin.url",
                project.sandbox
            ),
            0,
            "https://github.com/example-org/example-repo.git\n",
        )
        .answering(
            &format!(
                "exec {} -- git --git-dir {git_dir} config --get-all remote.origin.fetch",
                project.sandbox
            ),
            0,
            "+refs/heads/*:refs/remotes/origin/*\n",
        )
        .answering(
            &format!(
                "exec {} -- git --git-dir {git_dir} rev-parse refs/remotes/origin/main",
                project.sandbox
            ),
            0,
            &format!("{COMMIT}\n"),
        )
        .answering(
            &format!(
                "exec {} -- git -C {worktree} rev-parse HEAD",
                project.sandbox
            ),
            0,
            &format!("{COMMIT}\n"),
        )
        .answering(
            &format!(
                "exec {} -- git -C {worktree} rev-parse --path-format=absolute --git-common-dir",
                project.sandbox
            ),
            0,
            &format!("{git_dir}\n"),
        )
        .answering(
            &format!(
                "exec {} -- git -C {worktree} symbolic-ref -q HEAD",
                project.sandbox
            ),
            0,
            "refs/heads/main\n",
        );

    let output = run(
        &fixture.config,
        &project_id("example-org/example-repo"),
        &host,
        &fixture.workspace_root,
        poll(),
    )
    .expect("the fixed generation is completed");

    assert_eq!(output.applied, target);
    assert!(!output.unchanged);
    let stored = metadata::load(&project.paths).unwrap().expect("present");
    assert_eq!(stored.provisioning.dockerfile_sha256, target);
    assert!(
        stored.rebuild.is_none(),
        "the intent is cleared once everything verified"
    );
    assert!(
        !host.ran("image save"),
        "an image that is already built is not rebuilt: {:?}",
        host.calls()
    );
    assert!(
        host.ran("secret ls") && host.ran("ssh-add -L"),
        "the recreated sandbox reaches GitHub and not the host agent: {:?}",
        host.calls()
    );

    // 判定に使う出力はsbxmが読む。
    assert_eq!(host.spec("ls --json").output, OutputPolicy::Capture);
    assert_eq!(
        host.spec("template ls --json").output,
        OutputPolicy::Capture
    );
}
