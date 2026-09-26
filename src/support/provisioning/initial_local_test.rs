//! hostにあるrepositoryの案件は、hostのgitがssh越しにoriginを書き込んでSandboxを作る。

use crate::commands::add::AddRequest;
use crate::repository::RepositoryIdentity;
use crate::testing::outcome::{Checked, Required};
use crate::testing::provisioning::{Bench, World};

fn local_request() -> Checked<AddRequest> {
    Ok(AddRequest {
        repository: RepositoryIdentity::local("/home/user/code/app/.git", "app")
            .required_because("a local repository")?,
        worktrees: None,
        detach: None,
        start_branch: Some("main".to_string()),
        parent_inside_repository: false,
    })
}

#[test]
fn a_host_repository_is_pushed_into_the_sandbox_origin_from_the_host() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let output = bench
        .build(&world, &local_request()?)
        .required_because("the build completes")?;

    assert_eq!(output.project, "local/app");
    assert_eq!(output.start_ref, "main");
    let calls = world.invocations();
    let position = |needle: &str| {
        calls
            .iter()
            .position(|call| call.contains(needle))
            .required_because(&format!("no command matched {needle}: {calls:?}"))
    };
    // Sandboxの中から届くoriginは無い。hostのgitが、sshでbranchとtagを書き込む。
    // 書き込む先のsshは、Sandboxを作る前に確かめる。
    let origin = position("remote add origin sbxm-host::local/app")?;
    let branches = position("--prune +refs/heads/*:refs/remotes/origin/*")?;
    let tags = position("refs/tags/*:refs/tags/*")?;
    assert!(position("-G sbxm-local-app-")? < origin, "{calls:?}");
    assert!(origin < branches && branches < tags, "{calls:?}");
    assert!(
        calls[branches].contains("push --porcelain --no-verify ssh://sbxm-local-app-"),
        "{calls:?}"
    );
    assert!(
        !calls
            .iter()
            .any(|call| call.contains("fetch --prune --progress origin")),
        "the sandbox does not fetch from an origin it cannot reach: {calls:?}"
    );
    Ok(())
}

#[test]
fn a_host_repository_needs_no_github_token() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    bench
        .build(&world, &local_request()?)
        .required_because("the build completes")?;

    for call in world.invocations() {
        assert!(!call.contains("secret"), "no token is looked up: {call}");
        assert!(
            !call.contains("https://github.com"),
            "nothing reaches GitHub: {call}"
        );
        assert!(
            !call.contains("credential."),
            "no helper is written: {call}"
        );
        assert!(!call.contains("ls-remote"), "no remote is asked: {call}");
    }
    Ok(())
}

#[test]
fn a_built_host_repository_is_observed_as_ready() -> Checked {
    // 2度目の構築は、観測だけで完成を確かめて何も作らない。
    let bench = Bench::new()?;
    let world = World::new();
    let request = local_request()?;
    bench
        .build(&world, &request)
        .required_because("the first build completes")?;
    let mark = world.mark();

    let output = bench
        .ensure(
            &world,
            &crate::project::ProjectId::parse("local/app").required()?,
            &mut crate::design::SilentProgress,
        )
        .required_because("the second run completes")?;

    assert!(output.already_built, "{output:?}");
    assert!(
        !world.since(mark).iter().any(|call| call.contains("bundle")),
        "{:?}",
        world.since(mark)
    );
    Ok(())
}

/// hostに保存済みのbranchとして、`main`と`topic`を答える。
fn saved_on_the_host(world: &World) {
    let saved = "refs/sbx/sbxm-local-app-a888dc9878c3/heads";
    world.answering(
        "--format=%(refname) refs/sbx/",
        0,
        &format!("{saved}/main\n{saved}/topic\n"),
    );
}

/// Sandboxがsbxmの外で消えた状態にする。imageとtemplateは残る。
fn lose_sandbox(world: &World) {
    world.sandboxes.borrow_mut().clear();
    world.present.borrow_mut().clear();
    world.digests.borrow_mut().clear();
    world.contents.borrow_mut().clear();
    world.settings.borrow_mut().clear();
    world.repository.borrow_mut().clear();
    *world.bare_git_dir.borrow_mut() = None;
    world.worktrees.borrow_mut().clear();
}

#[test]
fn a_first_build_does_not_bring_back_branches_saved_under_the_same_name() -> Checked {
    // 同じ名前で前に登録した案件が保存したbranchは、この案件の作業ではない。登録した
    // ばかりの案件の初回構築は作り直しではなく、hostのbranchから始める。
    let bench = Bench::new()?;
    let world = World::new();
    saved_on_the_host(&world);

    let output = bench
        .build(&world, &local_request()?)
        .required_because("the build completes")?;

    assert!(output.restored.is_empty(), "{:?}", output.restored);
    assert!(
        !world.ran("sbxm/restore.bundle"),
        "{:?}",
        world.invocations()
    );
    Ok(())
}

#[test]
fn a_lost_sandbox_is_built_again_with_the_branches_saved_on_the_host() -> Checked {
    // Sandboxがsbxmの外で消えても、hostへ保存したbranchはhostに残っている。新しく作る
    // bare repositoryへ、worktreeより先に戻す。
    let bench = Bench::new()?;
    let world = World::new();
    bench
        .build(&world, &local_request()?)
        .required_because("the first build completes")?;
    lose_sandbox(&world);
    saved_on_the_host(&world);
    let mark = world.mark();

    let output = bench
        .ensure(
            &world,
            &crate::project::ProjectId::parse("local/app").required()?,
            &mut crate::design::SilentProgress,
        )
        .required_because("the sandbox is built again")?;

    assert_eq!(output.restored, ["main", "topic"]);
    let calls = world.since(mark);
    let position = |needle: &str| {
        calls
            .iter()
            .position(|call| call.contains(needle))
            .required_because(&format!("no command matched {needle}: {calls:?}"))
    };
    let restored = position("push --quiet --no-verify ssh://")?;
    assert!(
        position("+refs/heads/*:refs/remotes/origin/*")? < restored
            && restored < position("worktree add")?,
        "the branches come back after the origin is read and before the worktrees"
    );
    Ok(())
}

#[test]
fn a_restore_that_stopped_before_any_branch_came_back_is_done_again() -> Checked {
    // 作り直しが、bare repositoryを作ったあとで止まることがある。bare repositoryが在る
    // ことではなく、branchがまだ無いことから、次の実行で戻す。
    let bench = Bench::new()?;
    let world = World::new();
    bench
        .build(&world, &local_request()?)
        .required_because("the first build completes")?;
    let bare = world.bare_git_dir.borrow().clone().required()?;
    let repository = world.repository.borrow().clone();
    lose_sandbox(&world);
    world.present.borrow_mut().insert(bare.clone());
    *world.bare_git_dir.borrow_mut() = Some(bare);
    *world.repository.borrow_mut() = repository;
    saved_on_the_host(&world);

    let output = bench
        .ensure(
            &world,
            &crate::project::ProjectId::parse("local/app").required()?,
            &mut crate::design::SilentProgress,
        )
        .required_because("the sandbox is built again")?;

    assert_eq!(output.restored, ["main", "topic"]);
    Ok(())
}

#[test]
fn a_host_repository_records_the_token_it_does_not_need_as_not_applicable() -> Checked {
    // 無いtokenやhelperを、在るものとして記録しない。要らないものであり、欠けてもいない。
    use crate::support::observed::Observed;

    let bench = Bench::new()?;
    let world = World::new();
    bench
        .build(&world, &local_request()?)
        .required_because("the build completes")?;
    let project = crate::project::ProjectId::parse("local/app").required()?;
    let candidate = crate::support::select::find(&bench.location, &project).required()?;
    let metadata = candidate.reload().required()?;

    let observation = super::observe(
        &world,
        &candidate.paths,
        &bench.config,
        &metadata,
        bench.workspace_root.path(),
    )
    .required()?;

    for observed in [
        &observation.secret,
        &observation.credential_helper,
        &observation.token_env,
    ] {
        assert_eq!(observed, &Observed::NotApplicable);
    }
    assert!(observation.is_complete(), "{observation:?}");
    assert_eq!(Observed::NotApplicable.as_str(), "not-applicable");
    Ok(())
}
