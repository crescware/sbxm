//! GitHub tokenがSandboxへ入らないことと、tokenの形を問わず使えること。

use crate::testing::outcome::{Checked, Refused, Required};

use crate::diagnostics::ErrorId;
use crate::testing::add_request::request;
use crate::testing::provisioning::{Bench, World};

#[test]
fn the_build_never_asks_the_proxy_to_interpret_the_token() -> Checked {
    // sbxmはtokenをcustom secretとして登録させる。Docker Sandboxesの組み込み
    // `github` serviceへ登録させると、proxyのgithub presetがtokenの形で扱いを変え、
    // classic personal access tokenを注入しない。実機の測定（PR #10 / commit
    // 4cb8907）では、同じclassic tokenがSandboxの外から200、中から401になった。
    // custom secretはtokenの形を問わないため、classicでもfine-grainedでも通る。
    //
    // 構築のどの工程もservice secretへ触れないことを、実行したcommandの並びで固定する。
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the build completes")?;

    for call in world.invocations() {
        assert!(
            !call.contains("secret set ") || call.contains("secret set-custom"),
            "the token is never registered for a built-in service: {call}"
        );
        assert!(
            !call.contains("--service"),
            "nothing narrows the listing to a service: {call}"
        );
    }
    Ok(())
}

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
    // helperが持つのはplaceholderだけである。tokenそのものはSandboxへ入らない。
    let written = calls
        .iter()
        .find(|call| call.contains("credential.https://github.com.helper !f"))
        .required_because("the helper is written")?;
    assert!(
        written.contains("password=sbx-cs-"),
        "git presents the placeholder: {written}"
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
    // custom secretはSandboxの作成時に結び付く。先に作ってしまうと、登録しても
    // placeholderの届かないSandboxが残り、作り直しを強いることになる。
    assert!(
        !world.ran("sbx create"),
        "the sandbox is not created before the secret it has to be built with"
    );
    assert!(
        !world.ran("docker build"),
        "the image is not built before the missing secret is reported"
    );

    for host in crate::support::secret::GITHUB_HOSTS {
        world.secrets.borrow_mut().push(host.to_string());
    }
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
