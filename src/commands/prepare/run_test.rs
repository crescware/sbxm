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
        !world.since(mark).iter().any(|call| call.contains("build")
            || call.contains("image")
            || call.contains("create --name")),
        "nothing is built before the engine is confirmed reachable: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn a_sandbox_side_mutation_failure_carries_the_disk_state_at_that_moment() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;

    // files配置、identity設定、gh git_protocol設定、bare clone、worktree作成の
    // それぞれの代表的な失敗。
    for step in [
        "sbx cp --follow-link",
        "config --global user.name",
        "gh config set git_protocol",
        "git init --bare",
        "worktree add",
    ] {
        world.failing(step);
        let mark = world.mark();
        let error = bench
            .build(&world, &request)
            .refused_because(&format!("{step} fails"))?;
        let since = world.since(mark);
        world.nothing_fails();

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
fn an_already_loaded_template_is_reused_without_exporting_a_new_archive() -> Checked {
    // Templateが既にある経路では、archiveを作る理由がない。`docker image save`を
    // 呼ばずに済ませられるかを、中断からの再開に頼らず直接確かめる。
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
    bench
        .build(&world, &request)
        .required_because("the existing template is reused")?;

    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("image save")),
        "an archive is not exported when the template already exists: {:?}",
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
    let output = bench
        .build(&world, &request)
        .required_because("the same prepare finishes")?;

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
