//! 中断した構築を、同じ`prepare`が暗黙に継続しない。
use crate::testing::outcome::{Checked, Refused, Required};

use super::super::fake::{Bench, World};
use crate::diagnostics::ErrorId;
use crate::testing::add_request::request;

/// `add`と`prepare`が外部工程を呼ぶ順に並べた、失敗させる工程とその診断。
const STEPS: [(&str, ErrorId); 11] = [
    (
        "git clone --progress git@github.com",
        ErrorId::ExternalCommandFailed,
    ),
    ("docker build", ErrorId::ExternalCommandFailed),
    ("docker image save", ErrorId::ExternalCommandFailed),
    ("sbx template load", ErrorId::ExternalCommandFailed),
    ("sbx create", ErrorId::ExternalCommandFailed),
    ("sbx cp --follow-link", ErrorId::ExternalCommandFailed),
    ("config --global user.name", ErrorId::ExternalCommandFailed),
    ("sbx secret ls", ErrorId::ExternalCommandFailed),
    ("git init --bare", ErrorId::ExternalCommandFailed),
    ("check-ref-format", ErrorId::InvalidBranchName),
    ("worktree add", ErrorId::ExternalCommandFailed),
];

#[test]
fn an_interruption_at_any_mutating_step_closes_prepare_to_implicit_recovery() -> Checked {
    // 各中断を独立した世界で作る。intent保存後の次のprepareは、hostを変更せずに
    // 明示的なrecoveryが必要な状態として止まる。
    for (step, expected) in STEPS {
        let bench = Bench::new()?;
        let world = World::new();
        let request = request("Example-Org/Example-Repo", None, None)?;
        world.failing(step);
        let error = bench
            .build(&world, &request)
            .refused_because("the run stops at the step that failed")?;
        assert_eq!(error.first_id(), Some(expected), "{step}");

        // host cloneはprepareの前段、secret確認はintent保存前のread-only preflight
        // なので、どちらもpendingにはならない。
        if step != "git clone --progress git@github.com" && step != "sbx secret ls" {
            world.nothing_fails();
            let mark = world.mark();
            let pending = bench
                .build(&world, &request)
                .refused_because(&format!("{step}: prepare refuses implicit recovery"))?;
            assert_eq!(
                pending.first_id(),
                Some(ErrorId::InitialProvisioningPending),
                "{step}"
            );
            for mutation in [
                "docker build",
                "docker image save",
                "sbx template load",
                "sbx create",
                "sbx cp",
                "config --global user.name",
                "git init --bare",
                "worktree add",
            ] {
                assert!(
                    !world.since(mark).iter().any(|call| call.contains(mutation)),
                    "{step}: pending prepare must not run {mutation}: {:?}",
                    world.since(mark)
                );
            }
        }
    }
    Ok(())
}

#[test]
fn a_finished_build_is_a_no_op_for_the_same_add() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None)?;
    bench
        .build(&world, &request)
        .required_because("the first run builds")?;

    let mark = world.mark();
    let output = bench
        .build(&world, &request)
        .required_because("the second run changes nothing")?;

    assert!(output.already_built);
    for forbidden in [
        "docker build",
        "docker image save",
        "sbx template load",
        "sbx create",
        "sbx daemon stop",
        "sbx cp",
        "worktree add",
        "git clone",
    ] {
        assert!(
            !world
                .since(mark)
                .iter()
                .any(|call| call.contains(forbidden)),
            "a finished project must not run {forbidden}: {:?}",
            world.since(mark)
        );
    }
    Ok(())
}
