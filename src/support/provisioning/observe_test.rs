use std::fs;

use crate::diagnostics::{ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;
use crate::project::SandboxName;
use crate::support::Observed;
use crate::support::{image, select};
use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::provisioning::{Bench, World};

use super::{Observation, ProvisioningState, observe};

/// 構築済み案件を、lockを取らずにもう一度観測する。
fn observe_built(bench: &Bench, world: &World, project: &crate::project::ProjectId) -> Checked<()> {
    let candidate = select::find(&bench.location, project).required_because("find the project")?;
    let metadata = candidate.reload().required_because("read the metadata")?;
    let observation = observe(
        world,
        &candidate.paths,
        &bench.config,
        &metadata,
        bench.workspace_root.path(),
    )
    .required_because("observe the built project")?;
    assert_eq!(observation.state, ProvisioningState::Ready);
    Ok(())
}

#[test]
fn a_completed_project_is_observed_as_ready() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first build completes")?;

    observe_built(&bench, &world, &project_of(&request)?)
}

#[test]
fn a_stopped_sandbox_is_observed_as_unobservable_rather_than_incomplete() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first build completes")?;

    // 完成した案件のSandboxが止まっただけで、成果物は何も失われていない。
    world.stopped();
    let project = project_of(&request)?;
    let candidate = select::find(&bench.location, &project).required_because("find the project")?;
    let metadata = candidate.reload().required_because("read the metadata")?;

    let mark = world.mark();
    let observation = observe(
        &world,
        &candidate.paths,
        &bench.config,
        &metadata,
        bench.workspace_root.path(),
    )
    .required_because("observing a stopped project does not fail")?;

    assert_eq!(
        observation.state,
        ProvisioningState::Unobservable,
        "a stopped sandbox is not a partially built one"
    );
    observation
        .require_safe()
        .required_because("being stopped is not an unsafe observation")?;
    for observed in [
        &observation.repository,
        &observation.worktrees_present,
        &observation.identity,
        &observation.files_placed,
    ] {
        assert_eq!(observed.as_str(), "not-observed", "{observed:?}");
    }
    // 中を読むcommandはSandboxを起動し得る。observeはそれを1つも実行しない。
    assert!(
        !world.since(mark).iter().any(|call| call.contains("exec")),
        "{:?}",
        world.since(mark)
    );
    Ok(())
}

#[test]
fn only_the_missing_worktree_among_several_ends_the_matching_state() -> Checked {
    // 1本欠けているだけで走査を打ち切ると、存在するworktreeの一覧が空のまま返り、
    // 呼び出し側（`actions_for`など）が要求本数すべてを欠落として扱ってしまう。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", Some(2), Some("main"))?;
    bench
        .build(&world, &request)
        .required_because("the first build completes")?;

    let project = project_of(&request)?;
    let candidate = select::find(&bench.location, &project).required_because("find the project")?;
    let metadata = candidate.reload().required_because("read the metadata")?;
    let layout = crate::project::SandboxLayout::new(metadata.canonical_id());
    let missing = layout.worktree(1);
    world.present.borrow_mut().remove(&missing);
    world.worktrees.borrow_mut().remove(&missing);

    let observation = observe(
        &world,
        &candidate.paths,
        &bench.config,
        &metadata,
        bench.workspace_root.path(),
    )
    .required_because("one missing worktree does not end the observation")?;

    assert_eq!(observation.worktrees_present.as_str(), "missing");
    assert_eq!(
        observation.worktrees.len(),
        1,
        "the worktree that is still present is still reported: {:?}",
        observation.worktrees
    );
    assert_eq!(observation.worktrees[0].path, layout.worktree_name(0));
    Ok(())
}

#[test]
fn an_unsafe_artifact_is_recorded_without_ending_the_observation() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first build completes")?;

    // 同名のSandboxが、この案件のものではないworkspaceを指している。
    for row in world.sandboxes.borrow_mut().iter_mut() {
        row.workspace = "/tmp/somebody-elses-workspace".to_string();
    }
    let project = project_of(&request)?;
    let candidate = select::find(&bench.location, &project).required_because("find the project")?;
    let metadata = candidate.reload().required_because("read the metadata")?;

    let observation = observe(
        &world,
        &candidate.paths,
        &bench.config,
        &metadata,
        bench.workspace_root.path(),
    )
    .required_because("one unsafe artifact does not end the observation")?;

    assert!(
        matches!(observation.sandbox, Observed::Mismatch { .. }),
        "{:?}",
        observation.sandbox
    );
    // 観測は続き、他のartifactの事実も残る。
    assert!(observation.stored_image_present);
    let error = observation
        .require_safe()
        .refused_because("a mutation cannot start from an unsafe observation")?;
    assert!(error.contains_id(ErrorId::SandboxUnusable));
    Ok(())
}

#[test]
fn a_declared_file_edited_inside_the_sandbox_is_modified_rather_than_unsafe() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first build completes")?;

    // 利用者がSandboxの中で設定fileを書き換えた。壊れた成果物ではない。
    world.digests.borrow_mut().insert(
        "/home/agent/.config/example/settings.yaml".to_string(),
        crate::hash::sha256_hex(b"edited inside the sandbox\n"),
    );
    let project = project_of(&request)?;
    let candidate = select::find(&bench.location, &project).required_because("find the project")?;
    let metadata = candidate.reload().required_because("read the metadata")?;

    let observation = observe(
        &world,
        &candidate.paths,
        &bench.config,
        &metadata,
        bench.workspace_root.path(),
    )
    .required_because("observe the edited project")?;

    assert_eq!(observation.state, ProvisioningState::Ready);
    assert_eq!(observation.files_placed, Observed::Matching);
    assert_eq!(
        observation.files[0].placement,
        crate::support::files::Placement::Modified
    );
    observation
        .require_safe()
        .required_because("an edited file does not stop a mutation")?;
    Ok(())
}

/// 構築済みの案件で、`perturb`がSandboxの応答を変えたあとにlockを取らずに観測し直す。
///
/// `recorded`は、観測に渡すmetadataを読んだあとで変える。
fn reobserved(
    perturb: impl FnOnce(&World),
    recorded: impl FnOnce(&mut ProjectMetadata),
) -> Checked<Result<Observation>> {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first build completes")?;
    perturb(&world);

    let candidate = select::find(&bench.location, &project_of(&request)?)
        .required_because("find the project")?;
    let mut metadata = candidate.reload().required_because("read the metadata")?;
    recorded(&mut metadata);
    Ok(observe(
        &world,
        &candidate.paths,
        &bench.config,
        &metadata,
        bench.workspace_root.path(),
    ))
}

/// 確かめられなかったartifactは、拒否したerrorのidを証拠にして食い違いとして残り、
/// 観測は最後まで進んだうえで変更を止める。
fn blocked_by(observation: &Observation, observed: &Observed, id: ErrorId) -> Checked {
    assert_eq!(
        observed,
        &Observed::Mismatch {
            evidence: id.as_str().to_string()
        },
        "{observation:?}"
    );
    let error = observation
        .require_safe()
        .refused_because("a mutation cannot start from an artifact that was not confirmed")?;
    assert!(error.contains_id(id), "{error:?}");
    Ok(())
}

#[test]
fn a_git_identity_that_answers_someone_else_is_recorded_as_unsafe() -> Checked {
    let observation = reobserved(
        |world| world.answering("--get user.name", 0, "Someone Else\n"),
        |_| {},
    )?
    .required_because("a foreign identity does not end the observation")?;

    blocked_by(
        &observation,
        &observation.identity,
        ErrorId::SandboxIdentityMismatch,
    )
}

#[test]
fn a_credential_helper_that_cannot_be_read_in_time_is_recorded_as_unsafe() -> Checked {
    let observation = reobserved(|world| world.timing_out("--get credential."), |_| {})?
        .required_because("an unanswered probe does not end the observation")?;

    assert_eq!(observation.secret, Observed::Matching);
    blocked_by(
        &observation,
        &observation.credential_helper,
        ErrorId::ExternalCommandTimeout,
    )
}

#[test]
fn a_token_env_file_that_cannot_be_read_in_time_is_recorded_as_unsafe() -> Checked {
    let observation = reobserved(|world| world.timing_out("exec cat"), |_| {})?
        .required_because("an unanswered probe does not end the observation")?;

    assert_eq!(observation.credential_helper, Observed::Matching);
    blocked_by(
        &observation,
        &observation.token_env,
        ErrorId::ExternalCommandTimeout,
    )
}

#[test]
fn a_secret_listing_that_fails_leaves_the_helper_and_token_unconfirmed() -> Checked {
    // 登録を読めなければ、helperとtoken fileが正しいplaceholderを持つかも判定できない。
    // 推測で一致とせず、どちらも足りないものとして扱う。
    let observation = reobserved(|world| world.failing("secret ls"), |_| {})?
        .required_because("an unreadable registration does not end the observation")?;

    assert_eq!(observation.credential_helper, Observed::Missing);
    assert_eq!(observation.token_env, Observed::Missing);
    blocked_by(
        &observation,
        &observation.secret,
        ErrorId::ExternalCommandFailed,
    )
}

#[test]
fn a_tool_listing_that_fails_is_recorded_as_unsafe() -> Checked {
    let observation = reobserved(|world| world.failing("command -v"), |_| {})?
        .required_because("an unreadable tool list does not end the observation")?;

    blocked_by(
        &observation,
        &observation.tools,
        ErrorId::ExternalCommandFailed,
    )
}

#[test]
fn a_gh_protocol_that_is_not_set_is_missing_rather_than_unsafe() -> Checked {
    let observation = reobserved(|world| world.failing("config get git_protocol"), |_| {})?
        .required_because("an unset protocol does not end the observation")?;

    assert_eq!(observation.tools, Observed::Missing);
    observation
        .require_safe()
        .required_because("a setting that is not there yet is a gap, not a hazard")?;
    assert_eq!(observation.state, ProvisioningState::Incomplete);
    Ok(())
}

#[test]
fn a_gh_protocol_set_to_something_else_is_recorded_as_unsafe() -> Checked {
    let observation = reobserved(
        |world| world.answering("config get git_protocol", 0, "ssh\n"),
        |_| {},
    )?
    .required_because("a foreign protocol does not end the observation")?;

    blocked_by(
        &observation,
        &observation.tools,
        ErrorId::SandboxIdentityMismatch,
    )
}

#[test]
fn a_repository_that_is_not_bare_is_recorded_as_unsafe_and_its_worktrees_are_not_read() -> Checked {
    let observation = reobserved(
        |world| world.answering("rev-parse --is-bare-repository", 0, "false\n"),
        |_| {},
    )?
    .required_because("a foreign repository does not end the observation")?;

    assert!(observation.worktrees.is_empty(), "{observation:?}");
    blocked_by(
        &observation,
        &observation.repository,
        ErrorId::SandboxRepositoryUnusable,
    )
}

#[test]
fn a_repository_whose_presence_cannot_be_told_is_recorded_as_unsafe() -> Checked {
    let observation = reobserved(
        |world| world.answering("test -e /home/agent/work/example-repo/.git", 125, ""),
        |_| {},
    )?
    .required_because("an unanswered probe does not end the observation")?;

    blocked_by(
        &observation,
        &observation.repository,
        ErrorId::SandboxCheckUnobservable,
    )
}

#[test]
fn a_worktree_whose_presence_cannot_be_told_ends_only_the_worktree_scan() -> Checked {
    let observation = reobserved(
        |world| {
            world.answering(
                "test -e /home/agent/work/example-repo/example-repo.tree-0",
                125,
                "",
            );
        },
        |_| {},
    )?
    .required_because("an unanswered probe does not end the observation")?;

    assert_eq!(observation.repository, Observed::Matching);
    assert!(
        observation.worktrees.is_empty(),
        "no HEAD is read for a worktree that was not confirmed: {observation:?}"
    );
    blocked_by(
        &observation,
        &observation.worktrees_present,
        ErrorId::SandboxCheckUnobservable,
    )
}

#[test]
fn a_worktree_head_that_cannot_be_read_in_time_is_recorded_as_unsafe() -> Checked {
    let observation = reobserved(
        |world| world.timing_out("example-repo.tree-0 rev-parse HEAD"),
        |_| {},
    )?
    .required_because("an unanswered probe does not end the observation")?;

    blocked_by(
        &observation,
        &observation.worktrees_present,
        ErrorId::ExternalCommandTimeout,
    )
}

#[test]
fn a_worktree_that_belongs_to_another_repository_is_recorded_as_unsafe() -> Checked {
    let observation = reobserved(
        |world| world.answering("--git-common-dir", 0, "/home/agent/work/other/.git\n"),
        |_| {},
    )?
    .required_because("a foreign worktree does not end the observation")?;

    blocked_by(
        &observation,
        &observation.worktrees_present,
        ErrorId::SandboxRepositoryUnusable,
    )
}

#[test]
fn a_legacy_project_whose_declared_file_differs_inside_is_recorded_as_a_conflict() -> Checked {
    // baselineが無い案件では、中身の違うfileが利用者の編集なのか、壊れた成果物なのかを
    // 決められない。baselineがある案件と違い、書き換えられたfileとして受け入れない。
    let observation = reobserved(
        |world| {
            world.digests.borrow_mut().insert(
                "/home/agent/.config/example/settings.yaml".to_string(),
                crate::hash::sha256_hex(b"edited inside the sandbox\n"),
            );
        },
        |metadata| metadata.declared_files = None,
    )?
    .required_because("a conflicting file does not end the observation")?;

    assert!(observation.files.is_empty(), "{observation:?}");
    blocked_by(
        &observation,
        &observation.files_placed,
        ErrorId::DeclaredFileConflict,
    )
}

#[test]
fn two_sandboxes_under_the_projects_name_end_the_observation() -> Checked {
    // どちらがこの案件のSandboxか分からない。片方を選んで観測を続けない。
    let error = reobserved(
        |world| {
            let twin = world.sandboxes.borrow()[0].clone();
            world.sandboxes.borrow_mut().push(twin);
        },
        |_| {},
    )?
    .refused_because("the sandbox to observe cannot be told apart")?;

    assert_eq!(error.first_id(), Some(ErrorId::SandboxNameCollision));
    Ok(())
}

#[test]
fn a_template_listing_that_fails_is_not_read_as_an_absent_template() -> Checked {
    // Sandboxが消えた案件は、imageとTemplateを再利用できるかを見る。一覧を読めなかった
    // ことを、Templateが無いことと取り違えない。
    let error = reobserved(
        |world| {
            world.sandboxes.borrow_mut().clear();
            world.failing("template ls");
        },
        |_| {},
    )?
    .refused_because("whether the template is there cannot be told")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    Ok(())
}

/// 登録しただけの案件を、lockを取らずに観測する。
fn observe_registered(bench: &Bench, world: &World) -> Checked<Result<Observation>> {
    let request = request("Example-Org/Example-Repo", None, None)?;
    let project = bench
        .register(world, &request)
        .required_because("the project is registered")?;
    let candidate = select::find(&bench.location, &project).required_because("find the project")?;
    let metadata = candidate.reload().required_because("read the metadata")?;
    Ok(observe(
        world,
        &candidate.paths,
        &bench.config,
        &metadata,
        bench.workspace_root.path(),
    ))
}

#[test]
fn a_template_listing_that_fails_before_any_image_exists_ends_the_observation() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    world.failing("template ls");

    let error = observe_registered(&bench, &world)?
        .refused_because("whether a template is there cannot be told")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    Ok(())
}

#[test]
fn an_edited_dockerfile_whose_image_cannot_be_looked_up_ends_the_observation() -> Checked {
    // 保存済みの世代と編集後の世代の両方を見る。後の一方を読めなかっただけでも、
    // 無いこととして扱わない。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    let paths = ProjectPaths::derive(&bench.parent, request.repository.canonical_id());
    let edited = b"FROM example:edited\n";
    let current = image::image_name(
        &SandboxName::derive(request.repository.canonical_id()),
        &crate::hash::sha256_hex(edited),
    );
    bench
        .register(&world, &request)
        .required_because("the project is registered")?;
    fs::write(paths.dockerfile(), edited).required_because("edit the Dockerfile")?;
    world.failing(&format!("image ls --quiet {current}"));

    let candidate = select::find(&bench.location, &project_of(&request)?)
        .required_because("find the project")?;
    let metadata = candidate.reload().required_because("read the metadata")?;
    let error = observe(
        &world,
        &candidate.paths,
        &bench.config,
        &metadata,
        bench.workspace_root.path(),
    )
    .refused_because("whether the edited generation has an image cannot be told")?;

    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    assert!(
        world.ran("template ls"),
        "the stored generation was looked up first: {:?}",
        world.invocations()
    );
    Ok(())
}
