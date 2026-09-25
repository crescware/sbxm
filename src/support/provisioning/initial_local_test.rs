//! hostにあるrepositoryの案件は、hostから送ったbundleをoriginとしてSandboxを作る。

use crate::commands::add::AddRequest;
use crate::repository::RepositoryIdentity;
use crate::testing::outcome::{Checked, Required};
use crate::testing::provisioning::{Bench, World};

const BUNDLE: &str = "/home/agent/work/app/.git/sbxm/origin.bundle";

fn local_request() -> Checked<AddRequest> {
    Ok(AddRequest {
        repository: RepositoryIdentity::local("/home/user/code/app", "app")
            .required_because("a local repository")?,
        worktrees: None,
        detach: None,
        start_branch: Some("main".to_string()),
    })
}

#[test]
fn a_host_repository_is_built_from_a_bundle_sent_from_the_host() -> Checked {
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
    // originはSandboxの中に置いたbundleであり、hostのbranchとtagを送ってから取る。
    position(&format!("remote add origin {BUNDLE}"))?;
    assert!(
        position("bundle create --quiet")? < position("fetch --prune --progress origin")?,
        "the bundle reaches the sandbox before it is fetched"
    );
    assert!(
        world
            .contents
            .borrow()
            .get(BUNDLE)
            .is_some_and(|bytes| { String::from_utf8_lossy(bytes).contains("--branches --tags") }),
        "the sandbox holds the bundle of the host branches and tags"
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
