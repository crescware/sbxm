//! 中断した初回構築から復旧するまでを、commandをまたいで通す。
//!
//! `open`、`status`、`ls`、`repair`はそれぞれ別のcommandだが、利用者から見れば1つの
//! 導線である。個々のcommandのtestは、隣のcommandが同じ事実を同じ結論で扱っているか
//! までは見ない。ここでは1つの案件を1つのhostの上で進め、案内どおりに実行すれば
//! 復旧して接続まで届くことを固定する。

use crate::design::SilentProgress;
use crate::diagnostics::{ErrorId, Result};
use crate::project::ProjectId;
use crate::support::provisioning::{NextAction, ProvisioningState};

use crate::testing::add_request::request;
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::poll::poll;
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::provisioning::{Bench, World};

use super::open::run::Prepared;
use super::present::ListState;
use super::status::project::ProjectStatus;

const PROJECT: &str = "Example-Org/Example-Repo";

fn open(bench: &Bench, world: &World, project: &ProjectId) -> Result<Prepared> {
    super::open::run::prepare(
        &bench.location,
        &bench.config,
        Some(project),
        None,
        world,
        &mut ScriptedPrompt::choosing(0),
        bench.workspace_root.path(),
        poll(),
        &mut SilentProgress,
    )
}

fn diagnose(bench: &Bench, world: &World, project: &ProjectId) -> Checked<ProjectStatus> {
    super::status::project::diagnose(
        &bench.location,
        &bench.config,
        project,
        world,
        bench.workspace_root.path(),
    )
    .required_because("the project is diagnosed without changing it")
}

/// `ls`が同じ案件へ写した状態。
fn listed(bench: &Bench, world: &World) -> Checked<ListState> {
    let listing = super::ls::run::run(&bench.location, world, bench.workspace_root.path())
        .required_because("the listing is produced")?;
    Ok(listing
        .projects
        .iter()
        .find(|row| row.project == PROJECT)
        .required_because("the project is listed")?
        .state)
}

/// 案内された`repair`を、計画の確認から実行まで通す。
fn repair(bench: &Bench, world: &World, project: &ProjectId) -> Checked {
    let prepared = super::repair::run::prepare(
        &bench.location,
        &bench.config,
        Some(project),
        world,
        bench.workspace_root.path(),
        &mut ScriptedPrompt::choosing(0),
    )
    .required_because("the guided repair can be planned")?;
    assert_eq!(prepared.plan.state, ProvisioningState::Pending);

    let repaired = super::repair::run::execute(
        world,
        prepared,
        &bench.config,
        bench.workspace_root.path(),
        &mut SilentProgress,
    )
    .required_because("the guided repair finishes the build")?;
    assert!(repaired.changed);
    Ok(())
}

#[test]
fn an_interrupted_first_open_is_diagnosed_listed_repaired_and_then_opened() -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let project = bench
        .register(&world, &request(PROJECT, None, None)?)
        .required_because("the project is registered")?;

    // 1. 最初の`open`が、Sandboxを作る工程で中断する。
    world.failing("sbx create");
    let error =
        open(&bench, &world, &project).refused_because("the first mutation can be interrupted")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    world.nothing_fails();

    // 2. `status`が、実行できる1手として`repair`だけを名指しする。
    let status = diagnose(&bench, &world, &project)?;
    let next = status
        .next
        .required_because("the diagnosis ends with one command to run")?;
    assert_eq!(next, NextAction::RepairPending);
    assert_eq!(
        next.command(&status.project),
        "sbxm repair Example-Org/Example-Repo"
    );
    assert!(next.is_blocking(), "an unfinished build is not a success");

    // 3. `ls`が、同じ案件をopenできない状態として写す。
    assert_eq!(listed(&bench, &world)?, ListState::OpenBlocked);

    // 4. 案内された`repair`だけが、中断した工程の続きを進める。
    repair(&bench, &world, &project)?;
    assert!(
        bench.stored(PROJECT)?.initial_provisioning.is_none(),
        "a finished repair clears the intent"
    );

    // 5. 復旧後の`status`は何も案内せず、`ls`はrunningへ戻る。
    assert_eq!(
        diagnose(&bench, &world, &project)?.next,
        None,
        "a healthy project needs no command"
    );
    assert_eq!(listed(&bench, &world)?, ListState::Running);

    // 6. そのうえで`open`が、作り直さずに接続前検査まで進む。
    let mark = world.mark();
    let opened = open(&bench, &world, &project).required_because("the repaired project opens")?;
    assert!(
        opened.provisioned.is_none(),
        "a repaired project is not provisioned again"
    );
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("sbx create") || call.contains("docker build")),
        "nothing is built a second time: {:?}",
        world.since(mark)
    );
    Ok(())
}
