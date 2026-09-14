//! GitHub tokenがSandboxへ入らないこと。

use crate::testing::outcome::{Checked, Refused, Required};

use crate::diagnostics::ErrorId;
use crate::testing::add_request::request;
use crate::testing::provisioning::{Bench, World};

#[test]
fn git_is_given_the_placeholder_before_it_reaches_github() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the build completes")?;

    let calls = world.invocations();
    let position = |needle: &str| {
        calls
            .iter()
            .position(|call| call.contains(needle))
            .required_because(&format!("no command matched {needle}: {calls:?}"))
    };
    assert!(
        position("credential.https://github.com.helper")?
            < position("fetch --prune --progress origin")?,
        "a fetch without the credential asks for a username and never finishes"
    );
    Ok(())
}

#[test]
fn a_missing_secret_stops_the_build_and_the_same_add_continues_once_it_is_there() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    world.secrets.borrow_mut().clear();
    let request = request("Example-Org/Example-Repo", None, None)?;

    let error = bench
        .build(&world, &request)
        .refused_because("a build without repository access cannot continue")?;
    assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
    assert!(
        !world.ran("git init --bare"),
        "the sandbox repository is not made without the secret"
    );
    // tokenのないままSandboxを作らない。作ってから止まると、Templateのloadと作成の
    // 時間を使った末に、同じ案内で止まることになる。
    assert!(
        !world.ran("sbx create"),
        "the sandbox is not created before the secret it has to be built with"
    );
    assert!(
        !world.ran("docker build"),
        "the image is not built before the missing secret is reported"
    );

    world.secrets.borrow_mut().push("(global)".to_string());
    let output = bench
        .build(&world, &request)
        .required_because("the same add continues once the secret is registered")?;
    assert_eq!(output.worktrees.len(), 1);
    assert_eq!(
        world
            .invocations()
            .iter()
            .filter(|call| call.contains("sbx create"))
            .count(),
        1,
        "the sandbox that was already there is reused"
    );
    Ok(())
}
