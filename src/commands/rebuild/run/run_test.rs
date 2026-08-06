use std::path::Path;

use crate::command::{CommandOutcome, CommandSpec, HostEnvironment, OutputPolicy};
use crate::config::GlobalConfig;
use crate::diagnostics::{Error, ErrorId, Result};
use crate::metadata::{self, RebuildIntent};
use crate::msg;
use crate::project::SandboxLayout;
use crate::support::image;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::commands::rebuild::fake::verified;
use crate::commands::rebuild::{RebuildOutput, Target};
use crate::design::SilentProgress;
use crate::hash::{sha256_hex, short_hex};
use crate::paths::{self, LOCK_TIMEOUT, PRIVATE_FILE_MODE, PathScope};
use crate::testing::archive::image_archive_bytes;
use crate::testing::host::FakeSbx;
use crate::testing::image::template_listing;
use crate::testing::poll::poll;
use crate::testing::project::{Fixture, Registered, project_id};
use crate::testing::prompt::{ScriptedConfirm, ScriptedPrompt};
use crate::testing::protection::clean_host;
use crate::testing::value::{COMMIT, IMAGE_ID};
use std::os::unix::fs::PermissionsExt;

/// `exec`を模した、確認込みの実行。常に正しいSandbox名で確認する。
fn rebuild(
    target: Target,
    config: &GlobalConfig,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<RebuildOutput> {
    let (prepared, snapshot) = prepare(target, host, workspace_root, poll(), &mut SilentProgress)?;
    let sandbox = prepared.plan.sandbox.clone();
    let confirmation = confirm(snapshot, true, &mut ScriptedConfirm::typing(&sandbox))?;
    execute(
        host,
        prepared,
        confirmation,
        config,
        workspace_root,
        poll(),
        &mut SilentProgress,
    )
}

/// git、GitHub secret、Sandbox内部の検証まで一通り応答するhostを組み立てる。
fn ready_to_switch(host: FakeSbx, name: &str, git_dir: &str, worktree: &str) -> FakeSbx {
    verified(host, name)
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
        )
}

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
    let host = ready_to_switch(host, name, &git_dir, &worktree);

    let running = format!("[{}]", fixture.entry(&project, "running")?);
    let created = format!(
        r#"{{"sandboxes":[{{"name":"{}","status":"running","workspaces":["{}"]}}]}}"#,
        project.sandbox,
        workspace.display()
    );
    // 一覧は末尾から取り出される。状態の判定、削除前の再評価、削除完了の確認、作成前の
    // 確認までは稼働中のSandboxが対象と観測し、作成後は新しいSandboxを観測する。
    *host.listing.borrow_mut() = vec![
        created,
        "[]".to_string(),
        "[]".to_string(),
        running.clone(),
        running,
    ];

    let output = rebuild(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &fixture.config,
        &host,
        &fixture.workspace_root,
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
    // exclusive session leaseは、rebuildが終わったあとは保持されたままにならない。
    paths::acquire_exclusive_lock(
        &project.paths.session_lease_file(),
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("the exclusive session lease releases once the rebuild finishes")?;
    Ok(())
}

#[test]
fn an_open_session_stops_the_rebuild_before_anything_is_touched() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let session = paths::acquire_shared_lock(
        &project.paths.session_lease_file(),
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("simulate an active sbxm open session")?;

    let host = FakeSbx::listing("[]");
    let error = prepare(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .refused_because("a normal rebuild must not run while a session is open")?;
    assert_eq!(error.first_id(), Some(ErrorId::OpenSessionActive));
    assert!(
        host.calls().is_empty(),
        "the rebuild is refused before the host is touched at all: {:?}",
        host.calls()
    );

    drop(session);
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
    let error = prepare(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
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
    let error = prepare(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
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
    let _ = prepare(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
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
    let error = prepare(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
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

    let error = prepare(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .refused_because("a dirty worktree is not recreated")?;
    assert_eq!(error.first_id(), Some(ErrorId::WorktreeUntrackedPaths));
    assert!(
        !host.ran("build"),
        "the existing sandbox is untouched: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn only_the_exact_sandbox_name_confirms_an_interactive_rebuild() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    let host = clean_host(&fixture, &project)?;

    let (prepared, snapshot) = prepare(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .required_because("prepare")?;
    let sandbox = prepared.plan.sandbox.clone();

    let mut wrong = ScriptedConfirm::typing("yes");
    let error = confirm(snapshot, true, &mut wrong)
        .refused_because("only the sandbox name confirms a rebuild")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProtectionNotConfirmed));
    assert!(!host.ran("rm "), "nothing is removed without confirmation");
    let _ = sandbox;
    Ok(())
}

#[test]
fn a_non_interactive_run_is_refused_rather_than_skipped() -> Checked {
    // rebuildに`--force`は無い。非対話環境は、確認できないことを理由に拒否する。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    let host = clean_host(&fixture, &project)?;

    let (_prepared, snapshot) = prepare(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .required_because("prepare")?;

    let mut without_terminal = ScriptedConfirm::canceling();
    let error = confirm(snapshot, false, &mut without_terminal)
        .refused_because("a non-interactive run cannot confirm a rebuild")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProtectionNotConfirmed));
    assert_eq!(
        without_terminal.asked(),
        0,
        "no prompt is attempted without a terminal"
    );
    Ok(())
}

#[test]
fn a_resume_with_an_intent_still_shows_the_plan_and_asks_again() -> Checked {
    // #82: 中断した再構築の続きであっても、削除計画と明示確認を毎回省略しない。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    let target = sha256_hex(b"FROM scratch\n");

    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: target.clone(),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&project.paths, &metadata).required()?;

    let host = clean_host(&fixture, &project)?;
    let (prepared, snapshot) = prepare(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &host,
        &fixture.workspace_root,
        poll(),
        &mut SilentProgress,
    )
    .required_because("a resume still builds a plan")?;
    assert_eq!(prepared.plan.target_generation, target);

    // canceling the confirmation must not touch anything, even on a resume.
    let mut canceled = ScriptedConfirm::canceling();
    let error =
        confirm(snapshot, true, &mut canceled).refused_because("a resume still asks again")?;
    assert_eq!(error.exit_code(), crate::diagnostics::ExitCode::Canceled);
    assert!(
        !host.ran("build") && !host.ran("rm "),
        "nothing is mutated before the confirmation: {:?}",
        host.calls()
    );
    let stored = metadata::load(&project.paths)
        .required()?
        .required_because("present")?;
    assert_eq!(
        stored
            .rebuild
            .as_ref()
            .map(|intent| intent.target_dockerfile_sha256.as_str()),
        Some(target.as_str()),
        "the fixed generation is still there for the next attempt"
    );
    Ok(())
}

#[test]
fn a_sandbox_that_disappears_while_the_new_generation_is_prepared_fails_closed() -> Checked {
    // #82: confirmationは最初に観測したsnapshotのfingerprintへ結び付く。生成準備の
    // あいだに対象Sandboxが手作業で消えていれば、再評価したfingerprintは一致せず、
    // 黙って作成へ進まずに拒否する。
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
        .answering("template ls --json", 0, &template_listing(&image)?);

    let running = format!("[{}]", fixture.entry(&project, "running")?);
    // 一覧は末尾から取り出される。prepareの観測後、生成準備のあいだに対象Sandboxが
    // 手作業で消えている状況になる。
    *host.listing.borrow_mut() = vec!["[]".to_string(), running];

    let error = rebuild(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &fixture.config,
        &host,
        &fixture.workspace_root,
    )
    .refused_because("the confirmed state no longer matches what is there now")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProtectionStateChanged));

    assert!(
        !host.ran("rm ") && !host.ran("create --name"),
        "nothing is removed or created once the state has changed: {:?}",
        host.calls()
    );
    let stored = metadata::load(&project.paths)
        .required()?
        .required_because("present")?;
    assert!(
        stored.rebuild.is_none(),
        "a permit that was never issued commits no intent"
    );
    Ok(())
}

#[test]
fn a_new_generation_that_cannot_be_produced_leaves_the_existing_sandbox_alone() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    // buildは走るが、そのあともimageは一覧に現れない。
    let host = clean_host(&fixture, &project)?;

    let error = rebuild(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &fixture.config,
        &host,
        &fixture.workspace_root,
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
    let error = rebuild(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &fixture.config,
        &host,
        &fixture.workspace_root,
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

/// 固定した世代の成果物が揃い、再作成後の検証も通るhost。
///
/// 中断した`rebuild`の続きを、そのまま最後まで走らせられる状態を表す。
fn continuing(fixture: &Fixture, project: &Registered, target: &str) -> Checked<FakeSbx> {
    let image = image::image_name(&project.sandbox, target);
    let workspace = fixture.workspace_root.join(project.sandbox.as_str());
    std::fs::create_dir_all(&workspace).required()?;
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).required()?;
    let created = format!(
        r#"{{"sandboxes":[{{"name":"{}","status":"running","workspaces":["{}"]}}]}}"#,
        project.sandbox,
        workspace.display()
    );

    // 一覧は、prepare、execute再評価、作成前の確認、作成後の確認の順に読まれる。
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
        .answering("template ls --json", 0, &template_listing(&image)?);
    // 再作成後のSandbox内で、共有repositoryとworktreeが期待どおりに揃う。
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let git_dir = layout.bare_git_dir();
    let worktree = layout.worktree(0);
    Ok(ready_to_switch(
        host,
        project.sandbox.as_str(),
        &git_dir,
        &worktree,
    ))
}

#[test]
fn an_interrupted_rebuild_continues_from_the_generation_it_fixed() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    let target = sha256_hex(b"FROM scratch\n");

    // Sandbox削除の直後で中断した状態を作る。
    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: target.clone(),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&project.paths, &metadata).required()?;

    let host = continuing(&fixture, &project, &target)?;

    let output = rebuild(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &fixture.config,
        &host,
        &fixture.workspace_root,
    )
    .required_because("the fixed generation is completed")?;

    assert_eq!(output.applied, target);
    let stored = metadata::load(&project.paths)
        .required()?
        .required_because("present")?;
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
    assert_eq!(host.spec("ls --json")?.output(), OutputPolicy::Capture);
    assert_eq!(
        host.spec("template ls --json")?.output(),
        OutputPolicy::Capture
    );
    Ok(())
}

#[test]
fn an_edit_made_after_the_generation_was_fixed_is_left_for_the_next_rebuild() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let target = sha256_hex(b"FROM scratch\n");

    // 世代を固定したあとに、Dockerfileがさらに書き換えられた状態を作る。
    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: target.clone(),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&project.paths, &metadata).required()?;
    std::fs::write(project.paths.dockerfile(), "FROM alpine\n").required()?;

    let host = continuing(&fixture, &project, &target)?;
    let output = rebuild(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &fixture.config,
        &host,
        &fixture.workspace_root,
    )
    .required_because("the fixed generation is completed")?;

    assert_eq!(
        output.applied, target,
        "the generation that was fixed is the one that is applied"
    );
    assert_eq!(
        output
            .warnings
            .iter()
            .map(|warning| warning.description.id)
            .collect::<Vec<_>>(),
        vec!["warning-dockerfile-changed-during-rebuild"]
    );
    let stored = metadata::load(&project.paths)
        .required()?
        .required_because("present")?;
    assert_eq!(
        stored.provisioning.dockerfile_sha256, target,
        "the edit is not recorded as applied"
    );
    assert!(stored.rebuild.is_none());
    let edited = image::image_name(&project.sandbox, &sha256_hex(b"FROM alpine\n"));
    assert!(
        !host.ran(&edited),
        "the edit is left for the next rebuild: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_failure_after_the_switch_leaves_the_intent_in_place() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    let target = sha256_hex(b"FROM scratch\n");
    let previous = project.metadata.provisioning.dockerfile_sha256.clone();

    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: target.clone(),
        previous_dockerfile_sha256: previous.clone(),
    });
    metadata::update(&project.paths, &metadata).required()?;

    // 切り替えの最後の検査だけを落とす。作り直したSandboxからhostのSSH Agentへ届く。
    let host = continuing(&fixture, &project, &target)?.answering(
        &format!("exec {} -- ssh-add -L", project.sandbox),
        0,
        "ssh-ed25519 AAAA example\n",
    );

    let error = rebuild(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &fixture.config,
        &host,
        &fixture.workspace_root,
    )
    .refused_because("a sandbox that reaches the host agent is not accepted")?;
    assert_eq!(error.first_id(), Some(ErrorId::SshAgentExposed));

    let stored = metadata::load(&project.paths)
        .required()?
        .required_because("present")?;
    assert_eq!(
        stored
            .rebuild
            .as_ref()
            .map(|intent| intent.target_dockerfile_sha256.as_str()),
        Some(target.as_str()),
        "the fixed generation is still there, so a re-run continues from it"
    );
    assert_eq!(
        stored.provisioning.dockerfile_sha256, previous,
        "the generation is not applied until every check has passed"
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
        r#"{{"sandboxes":[{{"name":"{}","status":"running","workspaces":["{}"]}}]}}"#,
        project.sandbox,
        workspace.display()
    );

    let host = clean_host(&fixture, &project)?
        .answering(&format!("image ls --quiet {image}"), 0, "sha256:new\n")
        .answering(
            &format!("image inspect {image}"),
            0,
            &format!(
                r#"[{{"Id":"sha256:new","Config":{{"Labels":{{"io.crescware.sbxm.canonical-id":"example-org/example-repo","io.crescware.sbxm.dockerfile-sha256":"{target}","io.crescware.sbxm.metadata-version":"1"}}}}}}]"#
            ),
        )
        .answering("template ls --json", 0, &template_listing(&image)?);

    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let git_dir = layout.bare_git_dir();
    let worktree = layout.worktree(0);
    let name = project.sandbox.as_str();
    let host = ready_to_switch(host, name, &git_dir, &worktree);
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

    rebuild(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &fixture.config,
        &host,
        &fixture.workspace_root,
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

#[test]
fn a_sandbox_that_cannot_be_started_is_not_rebuilt_over() -> Checked {
    // 停止中のSandboxは、保存されていない作業を読むためにsbxmが起動する。その起動が
    // 通らない実行は、中を見ないまま作り直す工程へ進まない。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    let name = project.sandbox.as_str();
    let stopped = format!("[{}]", fixture.entry(&project, "stopped")?);
    let host = FakeSbx::listing(&stopped).answering(&format!("exec {name} -- /bin/true"), 1, "");

    let error = rebuild(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &fixture.config,
        &host,
        &fixture.workspace_root,
    )
    .refused_because("the sandbox will not start")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    assert!(
        !host.ran("build") && !host.ran("rm ") && !host.ran("worktree list"),
        "nothing is inspected or built before the sandbox answers: {:?}",
        host.calls()
    );
    let stored = metadata::load(&project.paths)
        .required()?
        .required_because("present")?;
    assert!(
        stored.rebuild.is_none(),
        "no generation is fixed before the sandbox could be read"
    );
    Ok(())
}

/// `docker image save`が書くarchiveを、指定されたpathへ実際に置くhost。
///
/// 新世代のTemplateはarchiveをloadして作る。archiveが置かれるところまで通さないと、
/// その工程は動かせない。
struct SavingSbx {
    inner: FakeSbx,
    image: String,
    labels: Vec<(String, String)>,
}

impl HostEnvironment for SavingSbx {
    fn command_exists(&self, program: &str) -> bool {
        self.inner.command_exists(program)
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        let outcome = self.inner.run(spec)?;
        if spec.args.first().is_some_and(|arg| arg == "image")
            && spec.args.get(1).is_some_and(|arg| arg == "save")
            && let Some(output) = spec.args.iter().skip_while(|arg| *arg != "--output").nth(1)
        {
            let labels: Vec<(&str, &str)> = self
                .labels
                .iter()
                .map(|(key, value)| (key.as_str(), value.as_str()))
                .collect();
            std::fs::write(output, image_archive_bytes(&self.image, IMAGE_ID, &labels)).map_err(
                |error| {
                    Error::new(
                        ErrorId::ArchiveUnusable,
                        msg!("error-archive-unusable", detail = error),
                    )
                },
            )?;
        }
        Ok(outcome)
    }
}

#[test]
fn the_first_rebuild_of_a_generation_exports_its_archive_and_loads_the_template() -> Checked {
    // 世代ごとのTemplateは、imageを保存したarchiveからloadして作る。既にあるTemplateを
    // 使い回す経路とは別に、まだ無い世代を作る経路がある。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    // 登録済み案件は、世代別archiveの置き場を持っている。
    std::fs::create_dir_all(project.paths.cache_dir()).required()?;
    let target = sha256_hex(b"FROM scratch\n");

    let mut metadata = project.metadata.clone();
    metadata.rebuild = Some(RebuildIntent {
        target_dockerfile_sha256: target.clone(),
        previous_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&project.paths, &metadata).required()?;

    let image = image::image_name(&project.sandbox, &target);
    // Templateはloadするまで一覧に現れない。
    let host = SavingSbx {
        inner: continuing(&fixture, &project, &target)?.answering_in_turn(
            "template ls --json",
            &[
                (0, r#"{"images":[]}"#),
                (0, r#"{"images":[]}"#),
                (0, &template_listing(&image)?),
            ],
        ),
        image: image.clone(),
        labels: image::expected_labels(project.metadata.canonical_id(), &target),
    };

    let output = rebuild(
        Target {
            location: &fixture.location,
            requested: Some(&project_id("example-org/example-repo")?),
            prompt: &mut ScriptedPrompt::choosing(0),
        },
        &fixture.config,
        &host,
        &fixture.workspace_root,
    )
    .required_because("the generation is produced and applied")?;

    assert_eq!(output.applied, target);
    assert!(
        host.inner.ran("image save") && host.inner.ran("template load"),
        "the archive is exported and loaded: {:?}",
        host.inner.calls()
    );
    // 検証を終えたarchiveだけが正式な位置へ移る。
    let archive = project.paths.template_archive(short_hex(&target));
    assert!(archive.is_file(), "the archive of the generation is kept");
    assert!(
        !project
            .paths
            .template_archive_temp(short_hex(&target))
            .exists(),
        "the temporary archive does not survive the run"
    );

    let stored = metadata::load(&project.paths)
        .required()?
        .required_because("present")?;
    assert_eq!(stored.provisioning.dockerfile_sha256, target);
    assert!(stored.rebuild.is_none());
    Ok(())
}
