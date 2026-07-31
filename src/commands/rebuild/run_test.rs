use crate::diagnostics::ErrorId;
use crate::metadata::{self, RebuildIntent};
use crate::project::SandboxLayout;
use crate::support::image;

use crate::testing::outcome::{Checked, Refused, Required};

use super::{super::fake::verified, *};
use crate::design::SilentProgress;
use crate::hash::sha256_hex;
use crate::testing::host::{FakeSbx, assert_lifecycle};
use crate::testing::image::template_listing;
use crate::testing::poll::poll;
use crate::testing::project::{Fixture, project_id};
use crate::testing::protection::clean_host;
use crate::testing::value::COMMIT;
use std::os::unix::fs::PermissionsExt;

#[test]
fn a_dockerfile_that_did_not_change_still_recreates_the_sandbox() -> Checked {
    let fixture = Fixture::new()?;
    let mut project = fixture.register("example-org/example-repo")?;
    // 適用済みhashと同じ内容のDockerfileを置く。
    std::fs::write(project.paths.dockerfile(), "unchanged\n").required()?;
    let target = sha256_hex(b"unchanged\n");
    project.metadata.provisioning.dockerfile_sha256 = target.clone();
    metadata::update(&project.paths, &project.metadata).required()?;

    let image = image::image_name(&project.sandbox, &target);
    let workspace = fixture.workspace_root.join(project.sandbox.as_str());
    std::fs::create_dir_all(&workspace).required()?;
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).required()?;

    let host = clean_host(&fixture, &project)?
        .answering(&format!("image ls --quiet {image}"), 0, "sha256:existing\n")
        .answering(
            &format!("image inspect {image}"),
            0,
            &format!(
                r#"[{{"Id":"sha256:existing","Config":{{"Labels":{{"io.crescware.sbxm.canonical-id":"example-org/example-repo","io.crescware.sbxm.dockerfile-sha256":"{target}","io.crescware.sbxm.metadata-version":"1"}}}}}}]"#
            ),
        )
        .answering("template ls --json", 0, &template_listing(&image)?);

    let layout = SandboxLayout::new(project.metadata.canonical_id());
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

    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let created = format!(
        r#"[{{"name":"{}","state":"running","workspace":"{}","template":"{image}","active_sessions":0}}]"#,
        project.sandbox,
        workspace.display()
    );
    // 一覧は末尾から取り出される。状態の判定、削除前の確認、削除完了の確認、作成前の
    // 確認までは稼働中のSandboxが対象と観測し、作成後は新しいSandboxを観測する。
    *host.listing.borrow_mut() = vec![
        created,
        "[]".to_string(),
        "[]".to_string(),
        running.clone(),
        running,
    ];

    let output = run(
        &fixture.location,
        &fixture.config,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .required_because("the same generation is applied again")?;

    assert_eq!(output.applied, target);
    assert!(
        !host.ran("build"),
        "the existing image is reused: {:?}",
        host.calls()
    );
    assert!(
        !host.ran("template load"),
        "the existing template is reused: {:?}",
        host.calls()
    );
    assert!(
        host.ran("rm ") && host.ran("create --name"),
        "an unchanged Dockerfile still recreates the sandbox: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_project_whose_build_never_finished_is_sent_to_add_even_with_the_same_dockerfile() -> Checked {
    let fixture = Fixture::new()?;
    let mut project = fixture.register("example-org/example-repo")?;
    // `add`は登録時に適用済みhashを書く。Sandboxを作る前に中断した案件は、
    // 現在のDockerfileと同じhashを持ったまま`not-created`で残る。
    std::fs::write(project.paths.dockerfile(), "unchanged\n").required()?;
    project.metadata.provisioning.dockerfile_sha256 = sha256_hex(b"unchanged\n");
    metadata::update(&project.paths, &project.metadata).required()?;

    let host = FakeSbx::listing("[]");
    let error = run(
        &fixture.location,
        &fixture.config,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .refused_because("there is no sandbox to report as unchanged")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotCreated));
    Ok(())
}

#[test]
fn a_project_that_is_not_managed_cannot_be_rebuilt() -> Checked {
    let fixture = Fixture::new()?;
    let host = FakeSbx::listing("[]");
    let error = run(
        &fixture.location,
        &fixture.config,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .refused_because("there is nothing to rebuild")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));
    Ok(())
}

#[test]
fn a_stopped_sandbox_is_started_rather_than_handed_back_to_the_user() -> Checked {
    // `rebuild`はこのSandboxをこれから作り直す。保存状態を読むためだけの起動を
    // 利用者へ求めない。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    let name = project.sandbox.as_str();

    let stopped = format!("[{}]", fixture.entry(&project, "stopped")?);
    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let host = FakeSbx::listings(&[&stopped, &running]);

    // 起動の先で止まってよい。ここで見たいのは、停止を理由に拒否しないことである。
    let _ = run(
        &fixture.location,
        &fixture.config,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    );

    assert!(
        host.ran(&format!("exec {name} -- /bin/true")),
        "the sandbox is started: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_project_without_a_sandbox_is_refused_with_the_command_that_helps() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;

    let absent = FakeSbx::listing("[]");
    let error = run(
        &fixture.location,
        &fixture.config,
        &project_id("example-org/example-repo")?,
        &absent,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .refused_because("a project without a sandbox has nothing to switch")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotCreated));
    assert!(!absent.ran("build"), "nothing is built");
    Ok(())
}

#[test]
fn unsaved_work_stops_the_rebuild_before_anything_is_built() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());

    let host = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
        0,
        "? scratch.txt\0",
    );

    let error = run(
        &fixture.location,
        &fixture.config,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .refused_because("a dirty worktree is not recreated")?;
    assert_eq!(error.first_id(), Some(ErrorId::UnsavedWork));
    assert!(
        !host.ran("build"),
        "the existing sandbox is untouched: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn the_sandbox_to_switch_is_decided_after_the_new_generation_is_ready() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    let target = sha256_hex(b"FROM scratch\n");
    let image = image::image_name(&project.sandbox, &target);
    let workspace = fixture.workspace_root.join(project.sandbox.as_str());
    std::fs::create_dir_all(&workspace).required()?;
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).required()?;

    let host = clean_host(&fixture, &project)?
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
                &template_listing(&image)?,
            );

    // 一覧は末尾から取り出される。世代の準備が終わるまでのあいだに、
    // 対象Sandboxが手作業で消された状況を作る。
    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let created = format!(
        r#"[{{"name":"{}","state":"running","workspace":"{}","template":"{image}","active_sessions":0}}]"#,
        project.sandbox,
        workspace.display()
    );
    // 一覧は末尾から取り出される。世代の準備が終わったあとの観測で、対象Sandboxが
    // 手作業で消えている状況になる。
    *host.listing.borrow_mut() = vec![created, "[]".to_string(), "[]".to_string(), running];

    let layout = SandboxLayout::new(project.metadata.canonical_id());
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
        &fixture.location,
        &fixture.config,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .required_because("the sandbox that is gone is created instead of removed")?;

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
    assert_lifecycle(&host, "create --name")?;
    Ok(())
}

#[test]
fn a_new_generation_that_cannot_be_produced_leaves_the_existing_sandbox_alone() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    // buildは走るが、そのあともimageは一覧に現れない。
    let host = clean_host(&fixture, &project)?;

    let error = run(
        &fixture.location,
        &fixture.config,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .refused_because("the new generation never became usable")?;
    assert_eq!(error.first_id(), Some(ErrorId::ImageUnusable));
    assert!(
        !host.ran("stop") && !host.ran("rm ") && !host.ran("create --name"),
        "the sandbox that is still running is untouched: {:?}",
        host.calls()
    );

    let stored = metadata::load(&project.paths)
        .required()?
        .required_because("present")?;
    assert!(
        stored.rebuild.is_none(),
        "no generation was fixed, so there is nothing to continue"
    );
    assert_eq!(
        stored.provisioning.dockerfile_sha256, project.metadata.provisioning.dockerfile_sha256,
        "the applied generation did not move"
    );
    Ok(())
}

#[test]
fn a_fixed_generation_with_neither_artifacts_nor_its_dockerfile_says_how_to_recover() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    // Dockerfileは、固定した世代とは別の内容へ変わっている。
    std::fs::write(project.paths.dockerfile(), "FROM alpine\n").required()?;

    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: sha256_hex(b"FROM scratch\n"),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&project.paths, &metadata).required()?;

    let host = clean_host(&fixture, &project)?;
    let error = run(
        &fixture.location,
        &fixture.config,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .refused_because("generations are never mixed")?;
    assert_eq!(error.first_id(), Some(ErrorId::RebuildGenerationMissing));

    let diagnostic = &error.diagnostics()[0];
    assert_eq!(
        diagnostic
            .remediation
            .as_ref()
            .and_then(|remediation| remediation.explanation.first())
            .map(|message| message.id),
        Some("remediation-rebuild-generation-missing")
    );
    assert!(
        !host.ran("build"),
        "the current Dockerfile is not built under the fixed generation's name: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_stopped_previous_generation_is_started_so_its_saved_state_can_be_read() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    let target = sha256_hex(b"FROM scratch\n");
    let previous = project.metadata.provisioning.dockerfile_sha256.clone();

    // Sandboxを削除する前に中断した状態。
    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: target.clone(),
        previous_dockerfile_sha256: previous.clone(),
    });
    metadata::update(&project.paths, &metadata).required()?;

    let image = image::image_name(&project.sandbox, &target);
    let workspace = fixture.workspace_root.join(project.sandbox.as_str());
    std::fs::create_dir_all(&workspace).required()?;
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).required()?;

    let stopped = format!("[{}]", fixture.entry(&project, "stopped")?);
    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let created = format!(
        r#"[{{"name":"{}","state":"running","workspace":"{}","template":"{image}","active_sessions":0}}]"#,
        project.sandbox,
        workspace.display()
    );
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).required()?;

    let host = clean_host(&fixture, &project)?
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
                &template_listing(&image)?,
            );

    let layout = SandboxLayout::new(project.metadata.canonical_id());
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
        &fixture.location,
        &fixture.config,
        &project_id("example-org/example-repo")?,
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .required_because("the fixed generation is completed from a stopped previous one")?;

    let calls = host.calls();
    let started = calls
        .iter()
        .position(|args| args.join(" ").contains("/bin/true"))
        .required_because("the stopped sandbox is started before it is inspected")?;
    let removed = calls
        .iter()
        .position(|args| args.first().is_some_and(|arg| arg == "rm"))
        .required_because("the previous generation is removed")?;
    assert!(
        started < removed,
        "the saved state is read from a running sandbox: {calls:?}"
    );
    Ok(())
}
