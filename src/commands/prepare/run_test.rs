use crate::metadata::{self};
use crate::paths::ProjectPaths;

use crate::testing::outcome::{Checked, Refused, Required};

use super::{
    super::fake::{Bench, World},
    *,
};
use crate::design::SilentProgress;
use crate::diagnostics::ErrorId;
use crate::hash::sha256_hex;
use crate::project::SandboxName;
use crate::testing::add_request::{project_of, request};
use crate::testing::project::project_id;
use crate::testing::prompt::ScriptedPrompt;
use std::fs;

#[test]
fn a_project_that_is_not_registered_is_sent_to_add() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();

    let error = run(
        &bench.location,
        &bench.config,
        Some(&project_id("example-org/example-repo")?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because("there is nothing to build yet")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));

    let diagnostic = &error.diagnostics()[0];
    assert_eq!(
        diagnostic
            .remediation
            .as_ref()
            .and_then(|remediation| remediation.explanation.first())
            .map(|message| message.id),
        Some("remediation-project-not-managed")
    );
    assert!(
        world.invocations().is_empty(),
        "nothing is asked of the host: {:?}",
        world.invocations()
    );
    Ok(())
}

#[test]
fn an_unregistered_project_gets_no_lock_file() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = project_id("example-org/example-repo")?;
    let paths = ProjectPaths::derive(&bench.parent, &project.canonical());
    // lock fileを置ける状態、つまりmetadataのない`.sbxm`だけがある状態で確かめる。
    fs::create_dir_all(paths.sbxm_dir())
        .required_because("the project directory is left behind")?;

    run(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because("there is nothing to build yet")?;

    assert!(
        !paths.lock_file().exists(),
        "an unregistered project is not given a lock file"
    );
    assert_eq!(
        fs::read_dir(paths.sbxm_dir())
            .required_because("read the project directory")?
            .count(),
        0,
        "nothing is written under an unregistered project"
    );
    Ok(())
}

#[test]
fn a_rebuild_in_progress_builds_nothing() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    crate::commands::add::run::run(
        &bench.location,
        &bench.parent,
        &request,
        &crate::testing::metadata::git_identity(),
        &world,
        &mut SilentProgress,
    )
    .required_because("the project is registered")?;

    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    let mut stored = metadata::load(&paths)
        .required_because("read the metadata")?
        .required_because("present")?;
    stored.rebuild = Some(metadata::RebuildIntent {
        target_dockerfile_sha256: sha256_hex(b"target"),
        previous_dockerfile_sha256: stored.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&paths, &stored).required_because("record the intent")?;

    let mark = world.mark();
    let error = run(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because("a half-switched project is not built on")?;
    assert_eq!(error.first_id(), Some(ErrorId::RebuildIntentPending));

    let remediation = error.diagnostics()[0]
        .remediation
        .as_ref()
        .required_because("the user is told how to get out of it")?;
    assert_eq!(remediation.explanation[0].id, "remediation-run-rebuild");
    // 実行するcommandは説明文ではなく、独立した一行として持つ。
    let command = remediation
        .commands
        .first()
        .required_because("the remediation carries the command to run")?;
    assert_eq!(command.as_str(), "sbxm rebuild Example-Org/Example-Repo");

    assert!(
        world.since(mark).is_empty(),
        "nothing is asked of the host: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn a_sandbox_that_is_not_this_projects_stops_prepare_instead_of_counting_as_built() -> Checked {
    // 「既に構築済みか」はSandbox identityまで確かめて決める。名前が同じだけの
    // Sandboxを構築済みとして扱えば、他人のworkspaceを案件の成果として見せてしまう。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first run builds it")?;

    // 同じ名前のSandboxが、この案件のものではないworkspaceを持っている。
    world
        .sandboxes
        .borrow_mut()
        .iter_mut()
        .for_each(|row| row.workspace = "/tmp/elsewhere".to_string());

    let mark = world.mark();
    let error = run(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because("a sandbox that cannot be identified is not the project's")?;

    assert_eq!(error.first_id(), Some(ErrorId::SandboxUnusable));
    assert!(
        !world.since(mark).iter().any(|call| call.contains("create")),
        "a second sandbox is not created over the one that is there: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn a_ready_project_is_a_no_op_even_when_docker_is_unreachable() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first prepare succeeds")?;

    world.failing("version --format");
    let output = run(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .required_because("a ready project does not need Docker")?;

    assert!(output.already_built);
    Ok(())
}

#[test]
fn a_foreign_image_stops_prepare_before_anything_is_built() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    crate::commands::add::run::run(
        &bench.location,
        &bench.parent,
        &request,
        &crate::testing::metadata::git_identity(),
        &world,
        &mut SilentProgress,
    )
    .required_because("the project is registered")?;

    let metadata = bench.stored("Example-Org/Example-Repo")?;
    let name = SandboxName::derive(request.repository.canonical_id());
    let image_name = image::image_name(&name, &metadata.provisioning.dockerfile_sha256);
    world.images.borrow_mut().insert(
        image_name,
        vec![(
            image::LABEL_CANONICAL_ID.to_string(),
            "Other-Org/Other-Repo".to_string(),
        )],
    );

    let error = run(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because("a foreign image is not overwritten")?;

    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningIncomplete)
    );
    assert!(!world.ran("docker build"));
    Ok(())
}

#[test]
fn an_image_that_cannot_be_inspected_leaves_the_generation_where_it_was() -> Checked {
    // どちらの世代で完成させるかは、保存済み世代のimageがあるかどうかで決まる。
    // それを観測できなかった実行は、どちらかへ倒さずに止まる。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;

    // imageまで組み上がり、Sandboxの作成で中断した実行を作る。
    world.failing("sbx create");
    bench
        .build(&world, &request)
        .refused_because("the run stops at sandbox creation")?;
    world.nothing_fails();

    let started_from = bench
        .stored("Example-Org/Example-Repo")?
        .provisioning
        .dockerfile_sha256;
    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    fs::write(paths.dockerfile(), b"FROM example:edited\n")
        .required_because("edit the Dockerfile")?;

    world.failing("docker image inspect");
    let mark = world.mark();
    let error = run(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because("the stored generation cannot be observed")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    assert_eq!(
        bench
            .stored("Example-Org/Example-Repo")?
            .provisioning
            .dockerfile_sha256,
        started_from,
        "an unobserved generation is not replaced by the edited one"
    );
    assert!(
        !world.since(mark).iter().any(|call| call.contains("build")),
        "nothing is built before the generation is decided: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn a_failed_image_build_requires_repair_even_after_the_dockerfile_is_fixed() -> Checked {
    // intentが保存された後は、Dockerfileを直してもprepareが新しいgenerationへ
    // 暗黙に切り替わらない。固定targetの成果物が無いので、repairも安全に止まる。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;

    world.failing("docker build");
    bench
        .build(&world, &request)
        .refused_because("the run stops when the image cannot be built")?;
    world.nothing_fails();
    let started_from = bench
        .stored("Example-Org/Example-Repo")?
        .provisioning
        .dockerfile_sha256;

    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    fs::write(paths.dockerfile(), b"FROM example:edited\n")
        .required_because("fix the Dockerfile")?;

    let error = bench
        .build(&world, &request)
        .refused_because("the same prepare does not retarget the fixed generation")?;
    assert_eq!(error.first_id(), Some(ErrorId::InitialProvisioningPending));
    let project = project_of(&request)?;
    let error = crate::commands::repair::run::prepare(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .refused_because("repair cannot invent the fixed target image")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningGenerationMissing)
    );
    let stored = bench.stored("Example-Org/Example-Repo")?;
    assert_eq!(
        stored.provisioning.dockerfile_sha256, started_from,
        "the original generation remains the target"
    );
    Ok(())
}

#[test]
fn an_engine_that_does_not_answer_stops_prepare_before_anything_is_built() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    crate::commands::add::run::run(
        &bench.location,
        &bench.parent,
        &request,
        &crate::testing::metadata::git_identity(),
        &world,
        &mut SilentProgress,
    )
    .required_because("the project is registered")?;

    world.failing("version --format");
    let mark = world.mark();
    let error = run(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because("without the engine there is nothing to prepare")?;

    assert_eq!(error.first_id(), Some(ErrorId::DockerUnreachable));
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("docker build") || call.contains("create --name")),
        "nothing is built before the engine is confirmed reachable: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn a_sandbox_side_mutation_failure_carries_the_disk_state_at_that_moment() -> Checked {
    // Issue #147が挙げる工程を、省略せず1つずつ失敗させる。archive save、Template
    // load、宣言file配置、Git identity、secret placeholder、credential helper、
    // bare clone、start ref解決、worktree作成。
    for step in [
        "docker image save",
        "template load",
        "sbx cp --follow-link",
        "config --global user.name",
        "gh config set git_protocol",
        "printf %s",
        "credential.https://github.com.helper",
        "git init --bare",
        "ls-remote --symref origin HEAD",
        "worktree add",
    ] {
        let bench = Bench::new()?;
        let world = World::new();
        let request = request("Example-Org/Example-Repo", None, None)?;
        world.failing(step);
        let mark = world.mark();
        let error = bench
            .build(&world, &request)
            .refused_because(&format!("{step} fails"))?;
        let since = world.since(mark);
        world.nothing_fails();

        assert_eq!(
            error.first_id(),
            Some(ErrorId::ExternalCommandFailed),
            "{step}: {error:?}"
        );
        assert!(
            bench
                .stored("Example-Org/Example-Repo")?
                .initial_provisioning
                .is_some(),
            "{step}: the intent survives the interruption"
        );
        assert!(
            since.iter().any(|call| call.contains(step)),
            "{step}: the injected failure was reached: {since:?}"
        );
    }
    Ok(())
}

#[test]
fn a_declaration_added_after_completion_is_not_placed_by_a_later_prepare() -> Checked {
    // 実測: 完成済みprojectのglobal configへ新しいdeclared fileを足すと、既存の実装は
    // Incompleteと判定してintentを作り、そのfileを配置する。repairの責務は初回構築が
    // 固定したbaselineの復旧であり、完成後に増えた宣言の配置は`apply`の責務である。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the project completes with its original declared file")?;

    let extra_home = tempfile::tempdir().required_because("temporary home for the extra file")?;
    let mut config = bench.config.clone();
    let extra_source = extra_home.path().join("extra.yaml");
    fs::write(&extra_source, b"extra = true\n").required_because("write the new file")?;
    config.files.push(crate::config::FileDeclaration {
        source: crate::config::HostFileSource::new(&crate::paths::display(&extra_source))
            .required_because("valid source")?,
        destination: crate::config::SandboxHomeRelativePath::new(".config/example/extra.yaml")
            .required_because("valid destination")?,
    });

    let mark = world.mark();
    let output = run(
        &bench.location,
        &config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .required_because("a declaration added after completion does not block the project")?;
    assert!(
        output.already_built,
        "the project stays ready; the new declaration is not this run's business"
    );
    assert!(
        !world.since(mark).iter().any(|call| call.contains("cp")),
        "nothing is copied for a declaration that was never part of the completed baseline: {:?}",
        world.since(mark)
    );
    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_none(),
        "no intent is created to chase a declaration that belongs to apply, not repair"
    );
    Ok(())
}

#[test]
fn a_legacy_project_without_a_recorded_baseline_is_ambiguous_when_a_declaration_is_missing()
-> Checked {
    // baselineを記録する前に完成した案件（この機能より前のversionが作った案件）で、
    // 宣言fileがsandboxに無い場合、それが構築の途中で欠けたのか、完成後に追加された
    // 宣言なのかをこの観測だけでは一意に決められない。intentを作って推測で復旧する
    // のではなく、拒否する。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the project completes")?;

    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    let mut stored = bench.stored("Example-Org/Example-Repo")?;
    stored.declared_files = None;
    metadata::update(&paths, &stored).required_because("simulate a pre-existing installation")?;

    let extra_home = tempfile::tempdir().required_because("temporary home for the extra file")?;
    let mut config = bench.config.clone();
    let extra_source = extra_home.path().join("extra.yaml");
    fs::write(&extra_source, b"extra = true\n").required_because("write the new file")?;
    config.files.push(crate::config::FileDeclaration {
        source: crate::config::HostFileSource::new(&crate::paths::display(&extra_source))
            .required_because("valid source")?,
        destination: crate::config::SandboxHomeRelativePath::new(".config/example/extra.yaml")
            .required_because("valid destination")?,
    });

    let error = run(
        &bench.location,
        &config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because(
        "a legacy project cannot tell broken from newly-declared without a baseline",
    )?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningBaselineAmbiguous)
    );
    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_none(),
        "no intent is guessed into existence for an ambiguous legacy project"
    );
    Ok(())
}

#[test]
fn a_legacy_project_without_a_recorded_baseline_stays_ready_when_nothing_changed() -> Checked {
    // baselineが無くても、現在の宣言がsandboxの中身とそのまま一致するなら健全とみなす。
    // この機能の追加が、既存installationの正常な案件を壊さないことを確かめる。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the project completes")?;

    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    let mut stored = bench.stored("Example-Org/Example-Repo")?;
    stored.declared_files = None;
    metadata::update(&paths, &stored).required_because("simulate a pre-existing installation")?;

    let output = run(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .required_because("an unchanged legacy project remains ready without a recorded baseline")?;
    assert!(output.already_built);
    Ok(())
}

#[test]
fn a_placeholder_no_longer_present_in_a_running_sandbox_is_not_treated_as_ready() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the project completes")?;

    // 何らかの理由で、稼働中のSandboxがもうplaceholderを持っていない。
    world.answering("printf %s", 0, "");
    let error = run(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because("a sandbox that lost its placeholder is not a verified post-condition")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningIncomplete)
    );
    Ok(())
}

#[test]
fn a_workspace_opened_up_to_group_and_other_is_not_treated_as_ready() -> Checked {
    // 実測: 完成済みprojectのworkspaceをmode 0777へ変更すると、既存の実装はready・
    // repair不要のまま成功する。存在するというだけでは安全とみなさず、群衆や他人が
    // 書き込めるdirectoryをそのまま採用しない。
    use std::os::unix::fs::PermissionsExt;

    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the project completes")?;

    let stored = bench.stored("Example-Org/Example-Repo")?;
    let workspace = bench
        .workspace_root
        .path()
        .join(stored.sandbox_name().as_str());
    fs::set_permissions(&workspace, fs::Permissions::from_mode(0o777))
        .required_because("open the workspace up to group and other")?;

    let error = run(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because("an overly-open workspace is not a verified post-condition")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::ProjectFilePermissionTooOpen)
    );
    Ok(())
}

#[test]
fn a_workspace_path_that_is_a_symlink_is_not_treated_as_a_reusable_artifact() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    crate::commands::add::run::run(
        &bench.location,
        &bench.parent,
        &request,
        &crate::testing::metadata::git_identity(),
        &world,
        &mut SilentProgress,
    )
    .required_because("the project is registered")?;

    let stored = bench.stored("Example-Org/Example-Repo")?;
    let workspace = bench
        .workspace_root
        .path()
        .join(stored.sandbox_name().as_str());
    let elsewhere = bench.workspace_root.path().join("elsewhere");
    fs::create_dir_all(&elsewhere).required_because("create a directory to link to")?;
    std::os::unix::fs::symlink(&elsewhere, &workspace)
        .required_because("point the workspace path at a symlink")?;

    let error = run(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because("a symlinked workspace path is not a verified post-condition")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
    Ok(())
}

#[test]
fn an_orphan_workspace_that_is_not_empty_is_not_treated_as_a_reusable_artifact() -> Checked {
    use std::os::unix::fs::PermissionsExt;

    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    crate::commands::add::run::run(
        &bench.location,
        &bench.parent,
        &request,
        &crate::testing::metadata::git_identity(),
        &world,
        &mut SilentProgress,
    )
    .required_because("the project is registered")?;

    let stored = bench.stored("Example-Org/Example-Repo")?;
    let workspace = bench
        .workspace_root
        .path()
        .join(stored.sandbox_name().as_str());
    fs::create_dir_all(&workspace).required_because("create the orphan workspace")?;
    fs::set_permissions(&workspace, fs::Permissions::from_mode(0o700))
        .required_because("keep the workspace private so only its contents are at issue")?;
    fs::write(workspace.join("leftover"), b"not sbxm's to explain")
        .required_because("leave unexplained content in it")?;

    let error = run(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because("a non-empty orphan workspace is not a safe artifact to build into")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxWorkspaceNotEmpty));
    Ok(())
}

#[test]
fn a_stale_archive_left_by_an_earlier_crash_is_swept_before_building() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    crate::commands::add::run::run(
        &bench.location,
        &bench.parent,
        &request,
        &crate::testing::metadata::git_identity(),
        &world,
        &mut SilentProgress,
    )
    .required_because("the project is registered")?;

    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    fs::create_dir_all(paths.cache_dir()).required_because("create the cache directory")?;
    let leftover = paths.cache_dir().join("template-000000000000.tar.tmp");
    fs::write(&leftover, b"left behind by an earlier crash")
        .required_because("write a leftover archive")?;

    run(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .required_because("prepare still succeeds")?;

    assert!(
        !leftover.exists(),
        "a stale archive from an earlier crash is swept while the lock is held"
    );
    Ok(())
}

#[test]
fn a_template_reused_by_name_alone_is_refused_when_its_runtime_id_differs() -> Checked {
    // 同じ名前のTemplateを無条件で再利用すると、別内容のTemplateへ同じ世代labelを
    // 付けたことになる。名前だけでなく、label検証済みhost imageから作るarchiveの
    // config digestとruntime idまで一致することを確かめてから再利用する。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;

    world.failing("docker image save");
    bench
        .build(&world, &request)
        .refused_because("the run stops before the archive is exported")?;
    world.nothing_fails();

    let stored = bench.stored("Example-Org/Example-Repo")?;
    let sandbox = SandboxName::derive(request.repository.canonical_id());
    let image = image::image_name(&sandbox, &stored.provisioning.dockerfile_sha256);
    // 何らかの理由で、この世代の名前を持つTemplateは既に存在するが、中身の対応は
    // 取れていない。
    world
        .templates
        .borrow_mut()
        .insert(image, "deadbeef".to_string());

    let mark = world.mark();
    let project = project_of(&request)?;
    let prepared = crate::commands::repair::run::prepare(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("repair observes the pending state before mutating")?;
    let error = crate::commands::repair::run::execute(
        &world,
        prepared,
        &bench.config,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .refused_because("a mismatched runtime id is not a verified reuse target")?;
    assert_eq!(error.first_id(), Some(ErrorId::TemplateUnusable));

    assert!(
        !world.since(mark).iter().any(|call| {
            call.contains("template load") || call.contains("sbx create") || call.contains("sbx cp")
        }),
        "a template whose id does not match is not loaded over, and nothing else mutates: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn a_template_with_a_matching_runtime_id_is_reused() -> Checked {
    // runtime idが実際に一致する場合は、名前一致だけの再利用と同じく、再loadしない。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;

    world.failing("worktree add");
    bench
        .build(&world, &request)
        .refused_because("the old run stopped after reusable artifacts were made")?;
    world.nothing_fails();

    let project = project_of(&request)?;
    let mark = world.mark();
    let prepared = crate::commands::repair::run::prepare(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("repair observes the pending state")?;
    crate::commands::repair::run::execute(
        &world,
        prepared,
        &bench.config,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .required_because("repair completes the interrupted build")?;

    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("template load")),
        "a template whose id matches is reused rather than reloaded: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn a_successful_prepare_never_asks_for_disk_usage() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;

    bench
        .build(&world, &request)
        .required_because("prepare succeeds")?;

    assert!(
        !world.ran("df -Pk"),
        "a successful run never checks disk usage: {:?}",
        world.invocations()
    );
    Ok(())
}

#[test]
fn a_successful_prepare_checks_secret_and_docker_reachability_exactly_once() -> Checked {
    // `provision`はrun側から確認済みの状態をconsumeするだけであり、内部で同じ
    // 外部callを二重に発行しない。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    crate::commands::add::run::run(
        &bench.location,
        &bench.parent,
        &request,
        &crate::testing::metadata::git_identity(),
        &world,
        &mut SilentProgress,
    )
    .required_because("the project is registered")?;

    let mark = world.mark();
    run(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .required_because("prepare succeeds")?;

    let since = world.since(mark);
    assert_eq!(
        since
            .iter()
            .filter(|call| call.contains("secret ls"))
            .count(),
        1,
        "the github secret is checked exactly once: {since:?}"
    );
    assert_eq!(
        since
            .iter()
            .filter(|call| call.contains("version --format"))
            .count(),
        1,
        "docker reachability is checked exactly once: {since:?}"
    );
    Ok(())
}

#[test]
fn a_workspace_that_had_to_be_created_again_is_told_rather_than_hidden() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    // worktreeを揃える工程で止め、Sandboxだけができている状態を作る。
    world.failing("worktree add");
    bench
        .build(&world, &request)
        .refused_because("the run stops at the step that failed")?;
    world.nothing_fails();

    // 続きを実行する前に、hostのworkspace directoryだけが消える。
    let sandbox = world.sandboxes.borrow()[0].name.clone();
    let workspace = bench.workspace_root.path().join(&sandbox);
    fs::remove_dir_all(&workspace).required_because("the workspace directory is removed")?;

    let mark = world.mark();
    let project = project_of(&request)?;
    let prepared = crate::commands::repair::run::prepare(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("repair resumes the interrupted build")?;
    let output = crate::commands::repair::run::execute(
        &world,
        prepared,
        &bench.config,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .required_because("repair finishes")?;

    assert!(
        workspace.is_dir(),
        "the mount point is there again: {}",
        workspace.display()
    );
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("sbx create")),
        "the sandbox itself is kept: {:?}",
        world.since(mark)
    );
    let restored = output
        .warnings
        .iter()
        .find(|warning| warning.description.id == "warning-workspace-restored")
        .required_because("creating the directory again is reported")?;
    assert_eq!(
        restored
            .facts
            .iter()
            .filter_map(|fact| match fact {
                crate::design::Fact::OneLine { value, .. } => Some(value.as_str().to_string()),
                _ => None,
            })
            .collect::<Vec<_>>(),
        vec![crate::paths::display(&workspace)],
        "the report names the directory it created"
    );
    Ok(())
}
