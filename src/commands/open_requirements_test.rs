//! `open`が守る要件を、commandをまたいで確かめる。
//!
//! 要件は`docs/design-principles.md`の「接続までの準備は`open`だけで済ませる」と、
//! 「Sandboxの側は安全に、hostの側は一意に決まるなら利便に倒す」に書いてある。
//!
//! 1. 接続までの準備は`open`だけで済む。どの工程で止まっても、`repair`を求めず、
//!    次の`open`が続きを済ませて接続する。
//! 2. `status`が`open`を案内した欠けは、`open`の1回の実行で直る。
//! 3. Sandboxの中で利用者が変えた設定を、`open`は戻さず、それを理由に止まらない。
//! 4. 初回構築を終えた案件で、GitHub tokenの登録が1件も無く、それ以外の準備が揃って
//!    いるなら、`open`は接続の前に、fetchとpushが認証で失敗することと登録するcommandを
//!    示して接続する。`status`はこの状態へ`open`を案内しない。

use crate::boundary::host::HostEnvironment;
use crate::design::SilentProgress;
use crate::diagnostics::ErrorId;
use crate::diagnostics::{Error, Result};
use crate::project::{ProjectId, SandboxLayout};
use crate::support::provisioning::NextAction;
use crate::support::secret::{GITHUB_HOSTS, register_command};

use crate::testing::add_request::request;
use crate::testing::host::FailingAt;
use crate::testing::outcome::{Checked, Required};
use crate::testing::poll::poll;
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::provisioning::{Bench, World};
use crate::testing::scripted_clock::ScriptedClock;

use super::open::run::Prepared;
use super::status::project::ProjectStatus;

const PROJECT: &str = "Example-Org/Example-Repo";

/// `Bench`が宣言するfileの、Sandbox内の置き場所。
const DECLARED_FILE: &str = "/home/agent/.config/example/settings.yaml";

fn open(bench: &Bench, host: &dyn HostEnvironment, project: &ProjectId) -> Result<Prepared> {
    let clock = ScriptedClock::default();
    super::open::run::prepare(
        &bench.location,
        &bench.config,
        Some(project),
        None,
        host,
        &mut ScriptedPrompt::choosing(0),
        bench.workspace_root.path(),
        poll(&clock),
        &mut SilentProgress,
    )
}

fn diagnose(bench: &Bench, world: &World, project: &ProjectId) -> Checked<ProjectStatus> {
    crate::testing::project::diagnose(
        &bench.location,
        &bench.config,
        project,
        world,
        bench.workspace_root.path(),
    )
    .required_because("the project is diagnosed without changing it")
}

fn registered(bench: &Bench, world: &World) -> Checked<ProjectId> {
    bench
        .register(world, &request(PROJECT, None, None)?)
        .required_because("the project is registered")
}

/// 一度`open`して完成させた案件。
fn built(bench: &Bench, world: &World) -> Checked<ProjectId> {
    let project = registered(bench, world)?;
    open(bench, world, &project).required_because("the first open builds")?;
    Ok(project)
}

/// 完成した案件から、1つを欠けさせる手順と、その名前。
type Gap = (&'static str, fn(&World, &SandboxLayout));

/// 完成した案件の、Sandboxの中の設定を1つ変える手順と、その名前。
type Change = (&'static str, fn(&World));

/// errorが次に実行するよう示すcommand。
fn suggested_commands(error: &Error) -> Vec<String> {
    error
        .diagnostics()
        .iter()
        .filter_map(|diagnostic| diagnostic.remediation.as_ref())
        .flat_map(|remediation| remediation.commands.iter())
        .map(|command| command.as_str().to_string())
        .collect()
}

#[test]
fn a_first_build_stopped_at_any_step_is_finished_by_the_next_open_alone() -> Checked {
    // 要件1。初回構築の`open`が起動する外部commandを、1つずつ時間切れにする。
    let steps = {
        let bench = Bench::new()?;
        let recorded = FailingAt::recording(World::new());
        let project = registered(&bench, &recorded.inner)?;
        open(&bench, &recorded, &project).required_because("every step answers")?;
        recorded.calls()
    };
    assert!(!steps.is_empty());

    let mut unfinished = Vec::new();
    for (at, step) in steps.iter().enumerate() {
        // 空き容量は、読めなかった理由として示すだけで、接続を止めない。
        if step.contains("df -Pk") {
            continue;
        }
        let bench = Bench::new()?;
        let failing = FailingAt::timing_out(World::new(), at);
        let project = registered(&bench, &failing.inner)?;
        let Err(error) = open(&bench, &failing, &project) else {
            continue;
        };

        let suggested = suggested_commands(&error);
        assert!(
            !suggested
                .iter()
                .any(|command| command.contains("sbxm repair")),
            "a stop at {step:?} must not send the user to repair: {suggested:?}"
        );
        if bench.stored(PROJECT)?.initial_provisioning.is_some() {
            assert!(
                suggested.contains(&format!("sbxm open {PROJECT}")),
                "a stop at {step:?} after the build started must name open: {suggested:?}"
            );
        }

        let world = &failing.inner;
        if let Err(error) = open(&bench, world, &project) {
            unfinished.push(format!("{step}: {:?}", error.first_id()));
            continue;
        }
        assert_eq!(
            diagnose(&bench, world, &project)?.next,
            None,
            "after a stop at {step:?}, one more open leaves nothing to do"
        );
    }
    assert!(
        unfinished.is_empty(),
        "the next open must finish the build after a stop at each of these steps:\n{}",
        unfinished.join("\n")
    );
    Ok(())
}

#[test]
fn every_gap_status_sends_to_open_is_closed_by_one_open() -> Checked {
    // 要件2。完成した案件から1つずつ欠けさせ、statusが案内したopenで直ることを見る。
    let gaps: [Gap; 7] = [
        ("Git identity", |world, _| {
            world.settings.borrow_mut().remove("user.name");
        }),
        ("gh git protocol", |world, _| {
            world.settings.borrow_mut().remove("git_protocol");
        }),
        ("credential helper", |world, _| {
            world
                .settings
                .borrow_mut()
                .remove("credential.https://github.com.helper");
        }),
        ("token environment file", |world, _| {
            world
                .settings
                .borrow_mut()
                .remove("/etc/profile.d/sbxm-github-token.sh");
        }),
        ("declared file", |world, _| {
            world.present.borrow_mut().remove(DECLARED_FILE);
            world.digests.borrow_mut().remove(DECLARED_FILE);
            world.contents.borrow_mut().remove(DECLARED_FILE);
        }),
        ("managed worktree", |world, layout| {
            let worktree = layout.worktree(0);
            world.present.borrow_mut().remove(&worktree);
            world.worktrees.borrow_mut().remove(&worktree);
        }),
        ("bare repository", |world, layout| {
            world.present.borrow_mut().remove(&layout.bare_git_dir());
            let worktree = layout.worktree(0);
            world.present.borrow_mut().remove(&worktree);
            world.worktrees.borrow_mut().remove(&worktree);
            world.repository.borrow_mut().clear();
            *world.bare_git_dir.borrow_mut() = None;
        }),
    ];

    for (gap, remove) in gaps {
        let bench = Bench::new()?;
        let world = World::new();
        let project = built(&bench, &world)?;
        let layout = SandboxLayout::new(bench.stored(PROJECT)?.canonical_id());
        remove(&world, &layout);

        let status = diagnose(&bench, &world, &project)?;
        assert_eq!(
            status.next,
            Some(NextAction::OpenIncomplete),
            "a missing {gap} is sent to open"
        );

        open(&bench, &world, &project).required_because("one open closes the gap")?;
        assert_eq!(
            diagnose(&bench, &world, &project)?.next,
            None,
            "after one open, the missing {gap} is back and nothing is left to do"
        );
    }
    Ok(())
}

#[test]
fn settings_changed_inside_the_sandbox_neither_stop_open_nor_are_reset() -> Checked {
    // 要件3。完成した案件で、利用者がSandboxの中の設定を1つずつ変える。
    let changes: [Change; 4] = [
        ("Git user name", |world| {
            world
                .settings
                .borrow_mut()
                .insert("user.name".to_string(), "Someone Else".to_string());
        }),
        ("Git user email", |world| {
            world
                .settings
                .borrow_mut()
                .insert("user.email".to_string(), "someone@example.com".to_string());
        }),
        ("gh git protocol", |world| {
            world
                .settings
                .borrow_mut()
                .insert("git_protocol".to_string(), "ssh".to_string());
        }),
        ("declared file", |world| {
            world.edited_inside(DECLARED_FILE, b"edited = true\n");
        }),
    ];

    for (setting, change) in changes {
        let bench = Bench::new()?;
        let world = World::new();
        let project = built(&bench, &world)?;
        change(&world);
        let settings = world.settings.borrow().clone();
        let digest = world.digests.borrow().get(DECLARED_FILE).cloned();

        open(&bench, &world, &project)
            .required_because("a setting changed inside the sandbox does not stop open")?;

        assert_eq!(
            *world.settings.borrow(),
            settings,
            "the changed {setting} is not reset"
        );
        assert_eq!(
            world.digests.borrow().get(DECLARED_FILE).cloned(),
            digest,
            "the changed {setting} is not reset"
        );
        assert_eq!(
            diagnose(&bench, &world, &project)?.next,
            None,
            "the changed {setting} is not reported as something to fix"
        );
    }
    Ok(())
}

/// `open`が接続の前に示す、tokenの登録が無いことの警告。
const TOKEN_WARNING: &str = "warning-open-github-token-missing";

/// 登録を解いた状態。Sandboxは残っているが、提示するtokenが無い。
fn unregister(world: &World) {
    world.secrets.borrow_mut().clear();
}

/// 登録し直した状態。
fn register(world: &World) {
    for host in GITHUB_HOSTS {
        world.secrets.borrow_mut().push((*host).to_string());
    }
}

/// この案件のtokenを登録するcommand。
fn registration(bench: &Bench) -> Checked<String> {
    Ok(register_command(
        bench.stored(PROJECT)?.sandbox_name().as_str(),
        None,
    ))
}

#[test]
fn a_missing_token_registration_alone_does_not_stop_open() -> Checked {
    // 要件4。
    let bench = Bench::new()?;
    let world = World::new();
    let project = built(&bench, &world)?;
    unregister(&world);
    let settings = world.settings.borrow().clone();

    assert_eq!(
        diagnose(&bench, &world, &project)?.next,
        None,
        "open cannot register the token, so status does not send this state to open"
    );

    let mark = world.mark();
    let prepared = open(&bench, &world, &project)
        .required_because("a missing registration alone does not stop open")?;

    let warning = prepared
        .warnings
        .iter()
        .find(|warning| warning.description.id == TOKEN_WARNING)
        .required_because("the missing registration is shown before connecting")?;
    assert_eq!(
        warning
            .commands
            .iter()
            .map(crate::design::text::CommandLine::as_str)
            .collect::<Vec<_>>(),
        vec![registration(&bench)?],
        "the warning names the command that registers the token"
    );
    // 補えるものは無い。補う工程へ進まず、Sandboxの中の設定も書き換えない。
    assert!(prepared.provisioned.is_none());
    assert_eq!(*world.settings.borrow(), settings);
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("ls-remote")),
        "{:?}",
        world.since(mark)
    );

    // 登録し直せば、次の`open`は警告せずに接続し、何も残らない。
    register(&world);
    let prepared = open(&bench, &world, &project)
        .required_because("the registered token is presented again")?;
    assert!(
        !prepared
            .warnings
            .iter()
            .any(|warning| warning.description.id == TOKEN_WARNING)
    );
    assert_eq!(diagnose(&bench, &world, &project)?.next, None);
    Ok(())
}

#[test]
fn a_missing_token_registration_with_a_missing_worktree_still_stops_open() -> Checked {
    // 要件4が扱わない場合。登録が無いうえに、`open`が補うmanaged worktreeも欠けている。
    // worktreeを作り直す工程は、起点のbranchのfetchにtokenを要ることがある。登録が
    // 無いことを理由に止まり、登録するcommandを示す。
    let bench = Bench::new()?;
    let world = World::new();
    let project = built(&bench, &world)?;
    let layout = SandboxLayout::new(bench.stored(PROJECT)?.canonical_id());
    let worktree = layout.worktree(0);
    world.present.borrow_mut().remove(&worktree);
    world.worktrees.borrow_mut().remove(&worktree);
    unregister(&world);

    assert_eq!(diagnose(&bench, &world, &project)?.next, None);
    let error = open(&bench, &world, &project)
        .err()
        .required_because("the worktree cannot be recreated without the token")?;
    assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
    assert!(
        suggested_commands(&error).contains(&registration(&bench)?),
        "{:?}",
        suggested_commands(&error)
    );

    register(&world);
    open(&bench, &world, &project).required_because("the registration lets open finish")?;
    assert_eq!(diagnose(&bench, &world, &project)?.next, None);
    Ok(())
}

#[test]
fn several_token_registrations_still_stop_open() -> Checked {
    // 要件4が扱わない場合。登録が複数あり、どれを使うか決まらない。登録すれば埋まる
    // 欠けではなく、どれを残すかを利用者が決める必要がある。今までどおり止まり、その
    // 理由と、余分な登録を消すcommandを示す。
    let bench = Bench::new()?;
    let world = World::new();
    let project = built(&bench, &world)?;
    let hosts = GITHUB_HOSTS.join("\",\"");
    world.answering(
        "secret ls --json",
        0,
        &format!(
            r#"{{"custom_secrets":[{{"scope":"(global)","targets":["{hosts}"],"env":"GH_TOKEN","placeholder":"sbx-cs-example"}},{{"scope":"(global)","targets":["{hosts}"],"env":"GH_TOKEN","placeholder":"sbx-cs-other"}}]}}"#
        ),
    );

    assert_eq!(diagnose(&bench, &world, &project)?.next, None);
    let error = open(&bench, &world, &project)
        .err()
        .required_because("which registration to present cannot be decided")?;
    assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
    assert!(
        suggested_commands(&error)
            .iter()
            .any(|command| command.starts_with("sbx secret rm")),
        "{:?}",
        suggested_commands(&error)
    );
    Ok(())
}
