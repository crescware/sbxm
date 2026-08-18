//! 中断した構築を、同じ`prepare`が暗黙に継続しない。
use crate::metadata::CreationMode;

use crate::testing::outcome::{Checked, Refused, Required};

use super::super::fake::{Bench, World};
use crate::compatibility::SandboxState;
use crate::diagnostics::ErrorId;
use crate::testing::add_request::{project_of, request};
use crate::testing::value::COMMIT;
use crate::{commands::repair::run::Prepared, design::SilentProgress};

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
fn every_interrupted_mutation_requires_repair_and_repair_finishes_it() -> Checked {
    for (step, expected) in STEPS {
        let bench = Bench::new()?;
        let world = World::new();
        let request = request("Example-Org/Example-Repo", None, None)?;
        world.failing(step);
        let error = bench
            .build(&world, &request)
            .refused_because("the run stops at the step that failed")?;
        assert_eq!(error.first_id(), Some(expected), "{step}");
        world.nothing_fails();

        // host cloneとsecret確認はintent保存前に失敗するため、通常のprepareで再試行できる。
        let output = if step == "git clone --progress git@github.com" || step == "sbx secret ls" {
            bench
                .build(&world, &request)
                .required_because(&format!("{step}: no recovery intent was committed"))?
        } else {
            let project = project_of(&request)?;
            let mark = world.mark();
            let pending = super::run(
                &bench.location,
                &bench.config,
                Some(&project),
                &world,
                bench.workspace_root.path(),
                &mut crate::testing::prompt::ScriptedPrompt::choosing(0),
                &mut SilentProgress,
            )
            .refused_because(&format!("{step}: prepare refuses implicit recovery"))?;
            assert_eq!(
                pending.first_id(),
                Some(ErrorId::InitialProvisioningPending),
                "{step}"
            );
            assert!(
                world.since(mark).is_empty(),
                "{step}: pending prepare performs no host action: {:?}",
                world.since(mark)
            );

            let prepared = crate::commands::repair::run::prepare(
                &bench.location,
                &bench.config,
                Some(&project),
                &world,
                &mut crate::testing::prompt::ScriptedPrompt::choosing(0),
                bench.workspace_root.path(),
            )?;
            let Prepared::Repairable(plan) = prepared else {
                return Err(crate::testing::outcome::Unmet::new(format!(
                    "{step}: the interrupted project is repairable"
                )));
            };
            crate::commands::repair::run::execute(
                *plan,
                &bench.config,
                &world,
                bench.workspace_root.path(),
                &mut SilentProgress,
            )?
        };

        assert!(!output.already_built, "{step}");
        assert_eq!(output.mode, CreationMode::Attached, "{step}");
        assert_eq!(output.start_ref, "main", "{step}");
        assert_eq!(output.sandbox_state, SandboxState::Running, "{step}");
        assert_eq!(output.worktrees.len(), 1, "{step}");
        assert_eq!(output.worktrees[0].path, "example-repo.tree-0", "{step}");
        assert_eq!(output.worktrees[0].head.as_deref(), Some(COMMIT), "{step}");
        assert!(
            bench
                .stored("Example-Org/Example-Repo")?
                .initial_provisioning
                .is_none(),
            "{step}: successful recovery commits completion"
        );
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
