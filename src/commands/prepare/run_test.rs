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
fn a_ready_project_with_a_stale_intent_requires_explicit_repair() -> Checked {
    // 完成直後のprocess interruption、または最後のintent消去だけの失敗を模し、
    // 完成済み成果物とintentが同時に残っている状態を作る。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first prepare succeeds")?;

    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    let mut stored = bench.stored("Example-Org/Example-Repo")?;
    stored.initial_provisioning = Some(metadata::InitialProvisioningIntent {
        target_dockerfile_sha256: stored.provisioning.dockerfile_sha256.clone(),
    });
    metadata::update(&paths, &stored).required_because("leave the stale intent behind")?;

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
    .refused_because("an intent is never cleared by ordinary prepare")?;

    assert_eq!(error.first_id(), Some(ErrorId::InitialProvisioningPending));
    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_some(),
        "prepare leaves the recovery intent for repair"
    );
    assert!(
        world.since(mark).is_empty(),
        "pending prepare does not inspect or mutate the host: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn incomplete_artifacts_require_explicit_repair() -> Checked {
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

    let name = SandboxName::derive(request.repository.canonical_id());
    let workspace = bench.workspace_root.path().join(name.as_str());
    fs::create_dir_all(&workspace).required_because("leave an unrecorded workspace behind")?;

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
    .refused_because("an incomplete project requires explicit repair")?;

    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningIncomplete)
    );
    let remediation = error.diagnostics()[0]
        .remediation
        .as_ref()
        .required_because("the user is told how to repair the project")?;
    assert_eq!(
        remediation.commands[0].as_str(),
        "sbxm repair Example-Org/Example-Repo"
    );
    assert!(
        world
            .since(mark)
            .iter()
            .all(|call| !call.contains("create") && !call.contains("build")),
        "observation does not mutate the host: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn an_image_collision_is_rejected_before_the_initial_intent_is_saved() -> Checked {
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

    assert_eq!(error.first_id(), Some(ErrorId::ImageUnusable));
    assert!(!world.ran("docker build"));
    Ok(())
}

#[test]
fn pending_prepare_does_not_inspect_an_image_or_change_its_generation() -> Checked {
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
    .refused_because("the interrupted run requires explicit repair")?;

    assert_eq!(error.first_id(), Some(ErrorId::InitialProvisioningPending));
    assert_eq!(
        bench
            .stored("Example-Org/Example-Repo")?
            .provisioning
            .dockerfile_sha256,
        started_from,
        "an unobserved generation is not replaced by the edited one"
    );
    assert!(
        world.since(mark).is_empty(),
        "pending prepare does not inspect or build anything: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn a_failed_image_build_cannot_be_retargeted_by_ordinary_prepare() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;

    world.failing("docker build");
    bench
        .build(&world, &request)
        .refused_because("the run stops when the image cannot be built")?;
    world.nothing_fails();

    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    fs::write(paths.dockerfile(), b"FROM example:edited\n")
        .required_because("fix the Dockerfile")?;

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
    .refused_because("the edited Dockerfile does not bypass explicit repair")?;

    assert_eq!(error.first_id(), Some(ErrorId::InitialProvisioningPending));
    let stored = bench.stored("Example-Org/Example-Repo")?;
    assert!(
        stored.initial_provisioning.is_some(),
        "prepare preserves the interrupted intent"
    );
    assert!(
        world.since(mark).is_empty(),
        "pending prepare neither retargets nor mutates: {:?}",
        world.since(mark)
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
        !world.since(mark).iter().any(|call| {
            call.contains("docker build")
                || call.contains("docker image save")
                || call.contains("sbx template load")
                || call.contains("sbx create")
        }),
        "read-only observation may run, but nothing is mutated before the engine is reachable: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn a_sandbox_side_mutation_failure_carries_the_disk_state_at_that_moment() -> Checked {
    // files配置、identity設定、gh git_protocol設定、bare clone、worktree作成の
    // それぞれの代表的な失敗。
    for step in [
        "sbx cp --follow-link",
        "config --global user.name",
        "gh config set git_protocol",
        "git init --bare",
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
        let facts = &error.diagnostics()[0].facts;
        assert_eq!(facts.len(), 4, "{step}: {facts:?}");
        assert_eq!(
            since.iter().filter(|call| call.contains("df -Pk")).count(),
            1,
            "{step}: exactly one disk check per failure: {since:?}"
        );
    }
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
fn an_existing_template_does_not_make_pending_prepare_resume() -> Checked {
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
    // 何らかの理由で、この世代のTemplateは既に存在する。
    world
        .templates
        .borrow_mut()
        .insert(image, "deadbeef".to_string());

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
    .refused_because("only repair may reuse the existing template")?;

    assert_eq!(error.first_id(), Some(ErrorId::InitialProvisioningPending));
    assert!(
        world.since(mark).is_empty(),
        "pending prepare does not inspect or export the template: {:?}",
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
fn pending_prepare_does_not_restore_a_missing_workspace() -> Checked {
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
    let error = run(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because("workspace restoration belongs to explicit repair")?;

    assert_eq!(error.first_id(), Some(ErrorId::InitialProvisioningPending));
    assert!(
        !workspace.exists(),
        "prepare leaves the missing mount point unchanged: {}",
        workspace.display()
    );
    assert!(
        world.since(mark).is_empty(),
        "pending prepare performs no host action: {:?}",
        world.since(mark)
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
