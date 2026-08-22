use super::*;

use crate::commands::prepare::fake::{Bench, World};
use crate::design::SilentProgress;
use crate::diagnostics::ErrorId;
use crate::hash::sha256_hex;
use crate::paths::ProjectPaths;
use crate::project::SandboxName;
use crate::support::files::Placement;
use crate::support::image;
use crate::support::provisioning::ProvisioningOutput;
use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::prompt::ScriptedPrompt;
use std::fs;

fn prepare_repair<'a>(
    bench: &'a Bench,
    world: &'a World,
    project: &'a crate::project::ProjectId,
) -> Checked<Prepared> {
    Ok(prepare(
        &bench.location,
        &bench.config,
        Some(project),
        world,
        &mut ScriptedPrompt::choosing(0),
        bench.workspace_root.path(),
    )?)
}

fn execute_repair(
    bench: &Bench,
    world: &World,
    project: &crate::project::ProjectId,
) -> Checked<ProvisioningOutput> {
    let prepared = prepare_repair(bench, world, project)?;
    let Prepared::Repairable(plan) = prepared else {
        return Err(crate::testing::outcome::Unmet::new(
            "the interrupted project is repairable",
        ));
    };
    Ok(execute(
        *plan,
        &bench.config,
        world,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )?)
}

#[test]
fn an_interrupted_prepare_is_fixed_only_by_explicit_repair() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("sbx create");
    bench
        .build(&world, &request)
        .refused_because("the initial provisioning stops at sandbox creation")?;
    world.nothing_fails();

    let project = project_of(&request)?;
    let mark = world.mark();
    let error = crate::commands::prepare::run::run(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
        &mut SilentProgress,
    )
    .refused_because("prepare never resumes an intent")?;
    assert_eq!(error.first_id(), Some(ErrorId::InitialProvisioningPending));
    assert!(
        world.since(mark).is_empty(),
        "pending prepare does not touch the host: {:?}",
        world.since(mark)
    );

    let output = execute_repair(&bench, &world, &project)?;
    assert!(!output.already_built);
    assert_eq!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning,
        None,
        "repair commits completion by removing the intent"
    );
    Ok(())
}

#[test]
fn a_fresh_project_is_not_a_repair_target() -> Checked {
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
    .required_because("register the project")?;
    let project = project_of(&request)?;

    let prepared = prepare_repair(&bench, &world, &project)?;
    assert!(matches!(prepared, Prepared::Fresh { .. }));
    assert_eq!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning,
        None
    );
    Ok(())
}

#[test]
fn a_healthy_project_is_a_noop_for_repair() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("build the project")?;
    let project = project_of(&request)?;

    let mark = world.mark();
    let prepared = prepare_repair(&bench, &world, &project)?;
    assert!(matches!(prepared, Prepared::Healthy { .. }));
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("create") || call.contains("worktree add")),
        "healthy repair never mutates: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn repair_clears_an_intent_left_after_every_artifact_was_completed() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("build every provisioning artifact")?;

    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    let mut metadata = bench.stored("Example-Org/Example-Repo")?;
    metadata.initial_provisioning = Some(crate::metadata::InitialProvisioningIntent {
        target_dockerfile_sha256: metadata.provisioning.dockerfile_sha256.clone(),
    });
    crate::metadata::update(&paths, &metadata)
        .required_because("simulate interruption before the final intent clear")?;

    let project = project_of(&request)?;
    let mark = world.mark();
    let output = execute_repair(&bench, &world, &project)?;

    assert!(!output.already_built);
    assert!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning
            .is_none(),
        "explicit repair commits the already completed generation"
    );
    assert!(
        !world.since(mark).iter().any(|call| {
            call.contains("docker build")
                || call.contains("sbx create")
                || call.contains("worktree add")
        }),
        "verified artifacts are reused: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn a_conflicting_image_is_rejected_before_repair_changes_metadata() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("sbx create");
    bench
        .build(&world, &request)
        .refused_because("leave an intent before sandbox creation")?;
    world.nothing_fails();

    let metadata = bench.stored("Example-Org/Example-Repo")?;
    let sandbox = metadata.sandbox_name();
    let image =
        crate::support::image::image_name(&sandbox, &metadata.provisioning.dockerfile_sha256);
    world.images.borrow_mut().insert(image, Vec::new());

    let project = project_of(&request)?;
    let mark = world.mark();
    let Err(error) = prepare(
        &bench.location,
        &bench.config,
        Some(&project),
        &world,
        &mut ScriptedPrompt::choosing(0),
        bench.workspace_root.path(),
    ) else {
        return Err(crate::testing::outcome::Unmet::new(
            "a mismatched image is diagnosed before the repair plan",
        ));
    };
    assert_eq!(error.first_id(), Some(ErrorId::ImageUnusable));
    assert_eq!(
        bench
            .stored("Example-Org/Example-Repo")?
            .initial_provisioning,
        metadata.initial_provisioning,
        "the diagnostic does not clear or rewrite the intent"
    );
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("sbx create")),
        "a conflicting image is diagnosed before Sandbox creation"
    );
    Ok(())
}

#[test]
fn repair_views_cover_fresh_healthy_and_repairable_states() -> Checked {
    let fresh_bench = Bench::new()?;
    let fresh_world = World::new();
    let fresh_request = request("Example-Org/Example-Repo", None, None)?;
    crate::commands::add::run::run(
        &fresh_bench.location,
        &fresh_bench.parent,
        &fresh_request,
        &crate::testing::metadata::git_identity(),
        &fresh_world,
        &mut SilentProgress,
    )
    .required_because("register a fresh project")?;
    let fresh = prepare_repair(&fresh_bench, &fresh_world, &project_of(&fresh_request)?)?;
    assert_eq!(fresh.view().phase, Phase::Fresh);

    let healthy_bench = Bench::new()?;
    let healthy_world = World::new();
    let healthy_request = request("Example-Org/Example-Repo", None, None)?;
    healthy_bench
        .build(&healthy_world, &healthy_request)
        .required_because("build a healthy project")?;
    let healthy = prepare_repair(
        &healthy_bench,
        &healthy_world,
        &project_of(&healthy_request)?,
    )?;
    assert_eq!(healthy.view().phase, Phase::Healthy);

    let repairable_bench = Bench::new()?;
    let repairable_world = World::new();
    let repairable_request = request("Example-Org/Example-Repo", None, None)?;
    repairable_world.failing("sbx create");
    repairable_bench
        .build(&repairable_world, &repairable_request)
        .refused_because("leave an interrupted project")?;
    repairable_world.nothing_fails();
    let repairable = prepare_repair(
        &repairable_bench,
        &repairable_world,
        &project_of(&repairable_request)?,
    )?;
    assert_eq!(repairable.view().phase, Phase::Plan);
    Ok(())
}

#[test]
fn a_nonempty_neutral_workspace_is_refused_before_repair() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("sbx create");
    bench
        .build(&world, &request)
        .refused_because("leave an interrupted project")?;
    world.nothing_fails();

    let metadata = bench.stored("Example-Org/Example-Repo")?;
    let workspace = bench
        .workspace_root
        .path()
        .join(metadata.sandbox_name().as_str());
    fs::create_dir_all(&workspace).required_because("create the neutral workspace")?;
    fs::write(workspace.join("unrecovered.txt"), b"keep me")
        .required_because("leave project data in the workspace")?;

    let Err(error) = prepare(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        &mut ScriptedPrompt::choosing(0),
        bench.workspace_root.path(),
    ) else {
        return Err(crate::testing::outcome::Unmet::new(
            "repair never overwrites a nonempty neutral workspace",
        ));
    };
    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningIncomplete)
    );
    Ok(())
}

#[test]
fn an_old_interrupted_project_without_intent_can_use_the_stored_generation() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("sbx create");
    bench
        .build(&world, &request)
        .refused_because("leave an interrupted project")?;
    world.nothing_fails();

    let mut metadata = bench.stored("Example-Org/Example-Repo")?;
    metadata.initial_provisioning = None;
    crate::metadata::update(
        &crate::paths::ProjectPaths::derive(&bench.parent, request.repository.canonical_id()),
        &metadata,
    )
    .required_because("remove the newer intent field")?;

    let prepared = prepare_repair(&bench, &world, &project_of(&request)?)?;
    assert!(matches!(prepared, Prepared::Repairable(_)));
    drop(prepared);
    Ok(())
}

#[test]
fn an_old_interrupted_project_uses_the_only_built_generation_after_an_edit() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("sbx create");
    bench
        .build(&world, &request)
        .refused_because("leave a built image from an interrupted project")?;
    world.nothing_fails();

    let mut metadata = bench.stored("Example-Org/Example-Repo")?;
    let stored = metadata.provisioning.dockerfile_sha256.clone();
    metadata.initial_provisioning = None;
    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    crate::metadata::update(&paths, &metadata).required_because("remove the newer intent field")?;
    fs::write(paths.dockerfile(), b"FROM example:edited\n")
        .required_because("edit the Dockerfile")?;

    let prepared = prepare_repair(&bench, &world, &project_of(&request)?)?;
    let Prepared::Repairable(plan) = prepared else {
        return Err(crate::testing::outcome::Unmet::new(
            "the only built generation is a safe repair target",
        ));
    };
    assert_eq!(plan.target_generation, stored);
    assert!(
        plan.warnings
            .iter()
            .any(|warning| warning.description.id == "warning-dockerfile-changed-during-build")
    );
    Ok(())
}

#[test]
fn an_old_interrupted_project_with_two_unbuilt_generations_is_unresolved() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("sbx create");
    bench
        .build(&world, &request)
        .refused_because("leave an interrupted project")?;
    world.nothing_fails();
    world.images.borrow_mut().clear();

    let mut metadata = bench.stored("Example-Org/Example-Repo")?;
    metadata.initial_provisioning = None;
    let paths =
        crate::paths::ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    crate::metadata::update(&paths, &metadata).required_because("remove the newer intent field")?;
    fs::create_dir_all(
        bench
            .workspace_root
            .path()
            .join(metadata.sandbox_name().as_str()),
    )
    .required_because("leave an observed workspace")?;
    fs::write(paths.dockerfile(), b"FROM example:edited\n")
        .required_because("change the Dockerfile")?;

    let Err(error) = prepare(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        &mut ScriptedPrompt::choosing(0),
        bench.workspace_root.path(),
    ) else {
        return Err(crate::testing::outcome::Unmet::new(
            "two generations without an image cannot be guessed",
        ));
    };
    assert_eq!(
        error.first_id(),
        Some(ErrorId::InitialProvisioningIncomplete)
    );
    Ok(())
}

#[test]
fn an_intent_that_disagrees_with_metadata_is_invalid() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("sbx create");
    bench
        .build(&world, &request)
        .refused_because("leave an interrupted project")?;
    world.nothing_fails();

    let mut metadata = bench.stored("Example-Org/Example-Repo")?;
    metadata.provisioning.dockerfile_sha256 = "f".repeat(64);
    let paths =
        crate::paths::ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    crate::metadata::update(&paths, &metadata).required_because("create an invalid intent")?;

    let Err(error) = prepare(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        &mut ScriptedPrompt::choosing(0),
        bench.workspace_root.path(),
    ) else {
        return Err(crate::testing::outcome::Unmet::new(
            "repair rejects an inconsistent target",
        ));
    };
    assert_eq!(error.first_id(), Some(ErrorId::InitialProvisioningInvalid));
    Ok(())
}

#[test]
fn a_changed_dockerfile_does_not_move_the_recorded_repair_target() -> Checked {
    const EDITED_DOCKERFILE: &[u8] = b"FROM example:edited\n";

    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("sbx create");
    bench
        .build(&world, &request)
        .refused_because("the run stops after building the image")?;
    world.nothing_fails();

    let started_from = bench
        .stored("Example-Org/Example-Repo")?
        .provisioning
        .dockerfile_sha256;
    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    fs::write(paths.dockerfile(), EDITED_DOCKERFILE).required_because("edit the Dockerfile")?;

    let mark = world.mark();
    let output = execute_repair(&bench, &world, &project_of(&request)?)?;

    assert_eq!(
        output
            .warnings
            .iter()
            .map(|warning| warning.description.id)
            .collect::<Vec<_>>(),
        vec!["warning-dockerfile-changed-during-build"]
    );
    assert_eq!(
        bench
            .stored("Example-Org/Example-Repo")?
            .provisioning
            .dockerfile_sha256,
        started_from
    );
    let edited = image::image_name(
        &SandboxName::derive(request.repository.canonical_id()),
        &sha256_hex(EDITED_DOCKERFILE),
    );
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("docker build") && call.contains(&edited)),
        "repair stays on the recorded target: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn an_identical_declared_file_is_not_written_again_by_repair() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("config --global user.name");
    bench
        .build(&world, &request)
        .refused_because("interrupt after placing the declared file")?;
    world.nothing_fails();

    let mark = world.mark();
    let output = execute_repair(&bench, &world, &project_of(&request)?)?;

    assert_eq!(output.files[0].placement, Placement::Unchanged);
    assert!(
        !world.since(mark).iter().any(|call| call.contains("sbx cp")),
        "an identical destination is left alone"
    );
    Ok(())
}

#[test]
fn an_unobservable_target_image_stops_before_repair_mutates() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("sbx create");
    bench
        .build(&world, &request)
        .refused_because("leave an interrupted project")?;
    world.nothing_fails();

    let metadata = bench.stored("Example-Org/Example-Repo")?;
    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    fs::write(paths.dockerfile(), b"FROM example:edited\n")
        .required_because("edit the Dockerfile")?;
    world.failing("docker image inspect");
    let mark = world.mark();

    let Err(error) = prepare(
        &bench.location,
        &bench.config,
        Some(&project_of(&request)?),
        &world,
        &mut ScriptedPrompt::choosing(0),
        bench.workspace_root.path(),
    ) else {
        return Err(crate::testing::outcome::Unmet::new(
            "repair refuses an unobservable image",
        ));
    };

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    assert_eq!(
        bench
            .stored("Example-Org/Example-Repo")?
            .provisioning
            .dockerfile_sha256,
        metadata.provisioning.dockerfile_sha256
    );
    assert!(
        !world.since(mark).iter().any(|call| call.contains("build")),
        "repair does not mutate before observation succeeds: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn an_existing_target_template_is_reused_without_exporting_an_archive() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("docker image save");
    bench
        .build(&world, &request)
        .refused_because("interrupt before exporting the archive")?;
    world.nothing_fails();

    let stored = bench.stored("Example-Org/Example-Repo")?;
    let sandbox = SandboxName::derive(request.repository.canonical_id());
    let image = image::image_name(&sandbox, &stored.provisioning.dockerfile_sha256);
    world
        .templates
        .borrow_mut()
        .insert(image, "deadbeef".to_string());

    let mark = world.mark();
    execute_repair(&bench, &world, &project_of(&request)?)?;

    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("image save")),
        "an existing template needs no archive: {:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn a_missing_neutral_workspace_is_restored_and_reported_by_repair() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    world.failing("worktree add");
    bench
        .build(&world, &request)
        .refused_because("interrupt after creating the Sandbox")?;
    world.nothing_fails();

    let sandbox = world.sandboxes.borrow()[0].name.clone();
    let workspace = bench.workspace_root.path().join(&sandbox);
    fs::remove_dir_all(&workspace).required_because("remove the neutral workspace")?;

    let mark = world.mark();
    let output = execute_repair(&bench, &world, &project_of(&request)?)?;

    assert!(workspace.is_dir(), "{}", workspace.display());
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("sbx create")),
        "repair keeps the existing Sandbox: {:?}",
        world.since(mark)
    );
    let restored = output
        .warnings
        .iter()
        .find(|warning| warning.description.id == "warning-workspace-restored")
        .required_because("the restored workspace is reported")?;
    assert!(
        restored.facts.iter().any(|fact| match fact {
            crate::design::Fact::OneLine { value, .. } =>
                value.as_str() == crate::paths::display(&workspace),
            _ => false,
        }),
        "the warning names {}",
        workspace.display()
    );
    Ok(())
}
