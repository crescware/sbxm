use crate::diagnostics::{Error, Result};
use crate::metadata::CreationMode;
use crate::paths::{self, PathScope, ProjectPaths};
use crate::project::SandboxLayout;
use crate::registry::RegistryGuard;
use crate::support::inventory::ProjectState;
use crate::support::select::{self};

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::command::{EnvPolicy, OutputPolicy, TimeoutClass};
use crate::design::SilentProgress;
use crate::diagnostics::{ErrorId, ExitCode};
use crate::metadata;
use crate::paths::{LOCK_TIMEOUT, PRIVATE_FILE_MODE};
use crate::testing::host::{FakeSbx, custom_secret_listing, no_custom_secrets, no_secrets};
use crate::testing::poll::poll;
use crate::testing::project::{Fixture, Registered, project_id};
use crate::testing::prompt::{ScriptedConfirm, ScriptedPrompt};
use crate::testing::protection::clean_host;
use std::os::unix::fs::PermissionsExt;

fn path_of(target: &Target) -> Option<&str> {
    match target {
        Target::Path(path) => Some(path.as_str()),
        Target::Described(_) => None,
    }
}

/// 説明で示す対象が計画に並んでいるか。
fn describes(plan: &DestroyPlan, id: &str) -> bool {
    plan.removes
        .iter()
        .chain(plan.keeps.iter())
        .any(|target| matches!(target, Target::Described(message) if message.id == id))
}

/// hostへ渡った指定の順番。工程の前後関係を検証する。
fn position(host: &FakeSbx, needle: &str) -> Checked<usize> {
    host.calls()
        .iter()
        .position(|args| args.join(" ").contains(needle))
        .required_because(&format!("no command matched {needle}"))
}

/// 削除が最後まで進む状況を作る。prepareの観測とremove直前の再評価までは対象が
/// 存在すると観測し、削除後の一覧では消えている。
fn expect_successful_removal(host: &FakeSbx) {
    let present = host.listing.borrow()[0].clone();
    *host.listing.borrow_mut() = vec![r#"{"sandboxes":[]}"#.to_string(), present.clone(), present];
}

/// `exec`を模した、確認込みの実行。通常modeは常に正しいSandbox名で確認する。
fn destroy(host: &FakeSbx, prepared: &mut Prepared) -> Result<DestroyOutcome> {
    let sandbox = prepared.plan.sandbox.clone();
    let confirmation = confirm(prepared, true, &mut ScriptedConfirm::typing(&sandbox))?;
    match confirmation {
        Some(confirmation) => {
            execute_confirmed(host, prepared, confirmation, poll(), &mut SilentProgress)
        }
        None => execute_bypassed(host, prepared, poll(), &mut SilentProgress),
    }
}

#[test]
fn a_clean_running_project_is_planned_then_removed() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("Example-Org/Example-Repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    std::fs::create_dir_all(project.paths.cache_dir()).required()?;
    // 削除後の一覧では対象が消えている。
    let host = no_secrets(clean_host(&fixture, &project)?, project.sandbox.as_str());
    expect_successful_removal(&host);

    let mut prepared = prepare(
        &fixture.location,
        Some(&project_id("Example-Org/Example-Repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .required_because("prepare")?;

    assert_eq!(prepared.plan.project, "Example-Org/Example-Repo");
    assert_eq!(prepared.plan.worktrees.len(), 1);
    assert!(
        prepared
            .plan
            .removes
            .iter()
            .any(|target| path_of(target).is_some_and(|path| path.contains("project.yaml")))
    );
    assert!(
        prepared
            .plan
            .keeps
            .iter()
            .any(|target| path_of(target).is_some_and(|path| path.contains("Dockerfile")))
    );
    assert_eq!(
        prepared.plan.re_register,
        "sbxm add git@github.com:Example-Org/Example-Repo.git --worktrees 1"
    );

    let outcome = destroy(&host, &mut prepared).required_because("destroy")?;
    assert!(outcome.warnings.is_empty());
    assert!(host.ran(&format!("rm {}", project.sandbox)));
    assert!(
        !host.ran("--force"),
        "the normal path never forces the removal: {:?}",
        host.calls()
    );
    assert!(
        !project.paths.metadata_file().exists(),
        "the project is unmanaged now"
    );
    assert!(!project.paths.cache_dir().exists());
    assert!(!project.paths.lock_file().exists());
    assert!(
        project.paths.dockerfile().exists(),
        "the Dockerfile the user edits is kept"
    );
    Ok(())
}

#[test]
fn the_removal_hides_sbxs_own_confirmation_and_the_listing_is_read_by_sbxm() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = no_secrets(clean_host(&fixture, &project)?, project.sandbox.as_str());
    expect_successful_removal(&host);

    let mut prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .required_because("prepare")?;
    destroy(&host, &mut prepared).required_because("destroy")?;

    // sbxmは既に自前の確認を済ませている。sbx自身の確認prompt/答えを利用者へ
    // 二重に見せない。
    let removal = host.spec(&format!("rm {}", project.sandbox))?;
    assert_eq!(
        removal.output(),
        OutputPolicy::Capture,
        "the runtime's own confirmation is answered internally, not shown to the user"
    );
    assert_eq!(removal.env, EnvPolicy::InheritWithoutSshAgent);
    assert_eq!(removal.timeout, TimeoutClass::SandboxLifecycle);

    // 判定に使う出力はsbxmが読む。
    let listing = host.spec("ls --json")?;
    assert_eq!(listing.output(), OutputPolicy::Capture);
    let inspection = host.spec("worktree list --porcelain -z")?;
    assert_eq!(inspection.output(), OutputPolicy::Capture);
    assert_eq!(inspection.env, EnvPolicy::InheritWithoutSshAgent);
    Ok(())
}

#[test]
fn a_runtime_refusal_of_the_removal_stops_before_the_listing_is_polled_again() -> Checked {
    // runtimeがactive sessionを理由に拒否した場合、exit statusが失われてはならず、
    // 一覧を再取得して不在を待つ工程（poll）へ進んではならない。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = no_secrets(clean_host(&fixture, &project)?, project.sandbox.as_str()).answering(
        &format!("rm {}", project.sandbox),
        1,
        "sandbox is in use",
    );

    let mut prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .required_because("prepare")?;
    let ls_calls_after_prepare = host
        .calls()
        .iter()
        .filter(|args| args.first().map(String::as_str) == Some("ls"))
        .count();

    let error = destroy(&host, &mut prepared)
        .refused_because("a runtime refusal is a removal failure, not something to retry")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));

    let ls_calls_after_execute = host
        .calls()
        .iter()
        .filter(|args| args.first().map(String::as_str) == Some("ls"))
        .count();
    // +1は、removeの直前に取り直す再評価用の1回だけである。それより増えていれば、
    // 拒否されたremovalのあとに不在を待つpollへ進んでしまっている。
    assert_eq!(
        ls_calls_after_execute,
        ls_calls_after_prepare + 1,
        "the listing is not polled again after a refused removal: {:?}",
        host.calls()
    );
    assert!(project.paths.metadata_file().exists());
    Ok(())
}

#[test]
fn a_stopped_project_is_refused_in_the_normal_mode_and_removed_with_force() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let stopped = format!(
        r#"{{"sandboxes":[{}]}}"#,
        fixture.entry(&project, "stopped")?
    );

    let host = FakeSbx::listing(&stopped);
    let error = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .refused_because("a stopped sandbox cannot be inspected")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxNotRunning));
    let remediation = error.diagnostics()[0]
        .remediation
        .as_ref()
        .required_because("a stopped sandbox has a safe recovery command")?;
    assert_eq!(
        remediation
            .commands
            .iter()
            .map(crate::design::text::CommandLine::as_str)
            .collect::<Vec<_>>(),
        vec!["sbxm open example-org/example-repo"]
    );

    let host = no_secrets(
        FakeSbx::listings(&[&stopped, r#"{"sandboxes":[]}"#]),
        project.sandbox.as_str(),
    );
    let mut prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        true,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .required_because("force skips the checks")?;
    assert!(prepared.plan.force);
    assert!(prepared.plan.worktrees.is_empty());

    destroy(&host, &mut prepared).required_because("destroy")?;
    // `--force`は、sbxm自身のデータ保護検査だけでなく、runtimeの確認prompt・
    // active-session検査も省く。通常経路とは別の`remove_forced`を通る。
    assert!(host.ran(&format!("rm --force {}", project.sandbox)));
    assert!(!project.paths.metadata_file().exists());
    Ok(())
}

#[test]
fn unsaved_work_stops_the_normal_mode_before_anything_is_deleted() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let name = project.sandbox.as_str();
    let managed = format!("{}/example-repo.tree-0", layout.bare_root());
    let host = clean_host(&fixture, &project)?.answering(
        &format!("exec {name} -- git -C {managed} status --porcelain=v2 -z --untracked-files=all"),
        0,
        "1 .M N... 100644 100644 100644 abc abc file.txt\0",
    );

    let error = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .refused_because("work that only exists here is not deleted")?;
    assert_eq!(error.first_id(), Some(ErrorId::WorktreeTrackedChanges));
    assert!(!host.ran("rm "), "nothing is removed");
    assert!(project.paths.metadata_file().exists());
    Ok(())
}

#[test]
fn an_open_session_stops_the_normal_destroy_before_anything_is_inspected() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = clean_host(&fixture, &project)?;
    let session = paths::acquire_shared_lock(
        &project.paths.session_lease_file(),
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("simulate an active sbxm open session")?;

    let error = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .refused_because("a normal destroy must not run while a session is open")?;
    assert_eq!(error.first_id(), Some(ErrorId::OpenSessionActive));
    assert!(
        !host.ran("git"),
        "the protection gate is never reached while a session is open: {:?}",
        host.calls()
    );
    assert!(!host.ran("rm "), "nothing is removed");
    assert!(project.paths.metadata_file().exists());

    drop(session);
    Ok(())
}

#[test]
fn force_bypasses_the_session_lease_even_while_a_session_is_open() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = no_secrets(clean_host(&fixture, &project)?, project.sandbox.as_str());
    host.listing
        .borrow_mut()
        .insert(0, r#"{"sandboxes":[]}"#.to_string());
    let session = paths::acquire_shared_lock(
        &project.paths.session_lease_file(),
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("simulate an active sbxm open session")?;

    prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        true,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .required_because("--force bypasses the session lease just like the protection gate")?;

    drop(session);
    Ok(())
}

#[test]
fn an_unmanaged_project_is_refused_before_the_host_is_touched() -> Checked {
    let fixture = Fixture::new()?;
    let host = FakeSbx::listing(r#"{"sandboxes":[]}"#);

    let error = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .refused_because("a project that is not managed has nothing to destroy")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));
    assert!(
        host.calls().is_empty(),
        "the host is not asked anything before the target is decided: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_project_without_a_sandbox_only_loses_its_management_data() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = no_secrets(
        FakeSbx::listing(r#"{"sandboxes":[]}"#),
        project.sandbox.as_str(),
    );

    let mut prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .required_because("prepare")?;
    assert_eq!(prepared.plan.state, ProjectState::NotCreated);

    destroy(&host, &mut prepared).required_because("destroy")?;
    assert!(!host.ran("rm "), "there is no sandbox to remove");
    assert!(!project.paths.metadata_file().exists());
    Ok(())
}

#[test]
fn the_token_registration_goes_away_with_the_sandbox() -> Checked {
    // scopeはSandboxの有無と無関係に残る。登録を残したまま管理を解くと、次に同じ案件を
    // addして案内どおりにset-customを実行しても、同じenvの登録が既にあるとして拒否される。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let sandbox = project.sandbox.as_str();
    let host = clean_host(&fixture, &project)?.answering_in_turn(
        &format!("secret ls {sandbox}"),
        &[
            (0, &custom_secret_listing(sandbox, "sbx-cs-example")),
            (0, &no_custom_secrets(sandbox)),
        ],
    );
    expect_successful_removal(&host);

    let mut prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .required_because("prepare")?;
    // 消す前に、tokenの登録も消えることを見せる。
    assert!(describes(&prepared.plan, "destroy-target-secret"));

    destroy(&host, &mut prepared).required_because("destroy")?;
    assert!(
        host.ran(&format!(
            "secret rm {sandbox} --placeholder sbx-cs-example --force"
        )),
        "the registration is named by its placeholder: {:?}",
        host.calls()
    );
    // 消し損ねたSandboxがplaceholderを持ったまま残る順序にしない。
    assert!(
        position(&host, &format!("rm {sandbox}"))? < position(&host, "secret rm")?,
        "the sandbox goes first, then the token it was given"
    );
    assert!(!project.paths.metadata_file().exists());
    Ok(())
}

#[test]
fn a_registration_that_survives_its_removal_keeps_the_project_managed() -> Checked {
    // commandの戻り値だけを不在の根拠にしない。残ったままなら、同じcommandでやり直せる
    // 状態を保ったまま停止する。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let sandbox = project.sandbox.as_str();
    let host = clean_host(&fixture, &project)?.answering(
        &format!("secret ls {sandbox}"),
        0,
        &custom_secret_listing(sandbox, "sbx-cs-example"),
    );
    expect_successful_removal(&host);

    let mut prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .required_because("prepare")?;

    let error =
        destroy(&host, &mut prepared).refused_because("the registration is still listed")?;
    assert_eq!(error.first_id(), Some(ErrorId::SecretStillRegistered));
    let remediation = error.diagnostics()[0]
        .remediation
        .as_ref()
        .required_because("the user is told how to remove it")?;
    assert!(
        remediation.commands.iter().any(|command| command.as_str()
            == format!("sbx secret rm {sandbox} --placeholder sbx-cs-example --force")),
        "the command that removes it is its own line: {remediation:?}"
    );
    assert!(
        project.paths.metadata_file().exists(),
        "the project stays managed so destroy can be run again"
    );
    Ok(())
}

#[test]
fn a_registration_of_another_scope_is_left_to_the_sandboxes_that_use_it() -> Checked {
    // global scopeのsecretはほかのSandboxも使う。1案件の後片付けで消す対象ではない。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let sandbox = project.sandbox.as_str();
    let host = clean_host(&fixture, &project)?.answering(
        &format!("secret ls {sandbox}"),
        0,
        &custom_secret_listing("global", "sbx-cs-elsewhere"),
    );
    expect_successful_removal(&host);

    let mut prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .required_because("prepare")?;

    destroy(&host, &mut prepared).required_because("destroy")?;
    assert!(
        !host.ran("secret rm"),
        "a registration this project did not own is untouched: {:?}",
        host.calls()
    );
    // ほかのSandbox向けのsecretは保持対象として示す。
    assert!(describes(&prepared.plan, "destroy-target-secrets"));
    Ok(())
}

#[test]
fn the_re_registration_command_repeats_the_target_configuration() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let mut metadata = project.metadata.clone();
    metadata.provisioning.mode = CreationMode::Detached;
    metadata.provisioning.start_ref = Some("develop".into());
    metadata.provisioning.requested_worktrees = 3;
    metadata::update(&project.paths, &metadata).required()?;

    assert_eq!(
        re_register(&project.paths, &metadata)
            .required_because("the target configuration is complete")?,
        "sbxm add git@github.com:example-org/example-repo.git --worktrees 3 --detach develop"
    );

    // 起点branchのないdetachedは再現できない。誤ったcommandを見せない。
    metadata.provisioning.start_ref = None;
    let error = re_register(&project.paths, &metadata)
        .refused_because("a configuration that cannot be repeated is not printed")?;
    assert_eq!(error.first_id(), Some(ErrorId::MetadataInvalidValue));
    Ok(())
}

#[test]
fn a_cache_that_is_a_symlink_is_not_followed_and_the_project_stays_managed() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let elsewhere = fixture
        .workspace_root
        .parent()
        .required()?
        .join("elsewhere");
    std::fs::create_dir_all(&elsewhere).required()?;
    std::fs::write(elsewhere.join("keep.txt"), "not ours\n").required()?;
    std::os::unix::fs::symlink(&elsewhere, project.paths.cache_dir()).required()?;

    let host = no_secrets(clean_host(&fixture, &project)?, project.sandbox.as_str());
    expect_successful_removal(&host);
    let mut prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .required_because("prepare")?;

    let error = destroy(&host, &mut prepared).refused_because("a symlinked cache is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
    assert!(
        elsewhere.join("keep.txt").exists(),
        "what the link pointed at is untouched"
    );
    assert!(
        project.paths.metadata_file().exists(),
        "the project stays managed so the state can be settled"
    );
    Ok(())
}

/// 削除して良い状態の案件を1件用意する。
fn prepared_project(fixture: &Fixture, force: bool) -> Checked<(FakeSbx, Prepared)> {
    let project = fixture.register("example-org/example-repo")?;
    let host = no_secrets(clean_host(fixture, &project)?, project.sandbox.as_str());
    host.listing
        .borrow_mut()
        .insert(0, r#"{"sandboxes":[]}"#.to_string());
    let prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        force,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .required_because("prepare")?;
    Ok((host, prepared))
}

#[test]
fn only_the_exact_sandbox_name_confirms_an_interactive_deletion() -> Checked {
    let fixture = Fixture::new()?;
    let (_host, mut prepared) = prepared_project(&fixture, false)?;
    let sandbox = prepared.plan.sandbox.clone();

    let mut exact = ScriptedConfirm::typing(&sandbox);
    let confirmation =
        confirm(&mut prepared, true, &mut exact).required_because("the name matched")?;
    assert!(confirmation.is_some());
    assert_eq!(exact.asked(), 1);

    // yesでは削除しない。名前以外の入力はすべて不一致とする。confirmationは1回の
    // snapshotだけを消費するため、それぞれ新しいpreparedで試す。
    for answer in ["yes", "", &sandbox[..sandbox.len() - 1]] {
        let fixture = Fixture::new()?;
        let (_host, mut prepared) = prepared_project(&fixture, false)?;
        let mut typed = ScriptedConfirm::typing(answer);
        let error = confirm(&mut prepared, true, &mut typed)
            .refused_because("only the sandbox name confirms a deletion")?;
        assert_eq!(error.first_id(), Some(ErrorId::ProtectionNotConfirmed));
    }
    Ok(())
}

#[test]
fn canceling_the_confirmation_changes_nothing_and_exits_130() -> Checked {
    let fixture = Fixture::new()?;
    let (host, mut prepared) = prepared_project(&fixture, false)?;

    let mut canceled = ScriptedConfirm::canceling();
    let error =
        confirm(&mut prepared, true, &mut canceled).refused_because("Esc and Ctrl-C leave")?;
    assert_eq!(error.exit_code(), ExitCode::Canceled);
    assert!(!host.ran("rm "), "nothing is removed");
    Ok(())
}

#[test]
fn force_mode_needs_no_confirmation() -> Checked {
    let fixture = Fixture::new()?;
    let (_host, mut forced) = prepared_project(&fixture, true)?;
    let mut with_terminal = ScriptedConfirm::canceling();
    let confirmation = confirm(&mut forced, true, &mut with_terminal)
        .required_because("force mode skips the confirmation")?;
    assert!(confirmation.is_none());
    assert_eq!(with_terminal.asked(), 0);
    Ok(())
}

#[test]
fn a_non_interactive_run_without_force_is_refused_rather_than_skipped() -> Checked {
    // 非対話環境は、確認できないことを理由に拒否する。forceと同列に黙って進めない。
    let fixture = Fixture::new()?;
    let (_host, mut normal) = prepared_project(&fixture, false)?;
    let mut without_terminal = ScriptedConfirm::canceling();
    let error = confirm(&mut normal, false, &mut without_terminal)
        .refused_because("a non-interactive run cannot confirm a deletion")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProtectionNotConfirmed));
    assert_eq!(
        without_terminal.asked(),
        0,
        "no prompt is attempted without a terminal"
    );
    Ok(())
}

#[test]
fn the_project_lock_is_held_across_the_confirmation() -> Checked {
    let fixture = Fixture::new()?;
    let (_host, prepared) = prepared_project(&fixture, false)?;
    let lock_file = ProjectPaths::derive(
        &fixture.parent,
        &project_id("example-org/example-repo")?.canonical(),
    )
    .lock_file();

    // 確認を待つあいだも、別の実行はこの案件へ入れない。
    let waiting = std::time::Duration::from_millis(200);
    paths::acquire_exclusive_lock(
        &lock_file,
        waiting,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .refused_because("another run cannot reach the project while the confirmation is pending")?;

    // lockはPreparedとともに解放される。
    drop(prepared);
    paths::acquire_exclusive_lock(
        &lock_file,
        waiting,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("the lock is released with Prepared")?;
    Ok(())
}

#[test]
fn the_session_lease_is_held_across_the_confirmation() -> Checked {
    let fixture = Fixture::new()?;
    let (_host, prepared) = prepared_project(&fixture, false)?;
    let lease_file = ProjectPaths::derive(
        &fixture.parent,
        &project_id("example-org/example-repo")?.canonical(),
    )
    .session_lease_file();

    // 確認を待つあいだも、通常rebuild/destroyのexclusive session leaseはここへ入れない。
    let waiting = std::time::Duration::from_millis(200);
    paths::acquire_exclusive_lock(
        &lease_file,
        waiting,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .refused_because(
        "another normal destroy/rebuild cannot proceed while the confirmation is pending",
    )?;

    // leaseはPreparedとともに解放される。
    drop(prepared);
    paths::acquire_exclusive_lock(
        &lease_file,
        waiting,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )
    .required_because("the session lease is released with Prepared")?;
    Ok(())
}

/// `.sbxm`から書き込みを取り上げ、cleanupを失敗させる。
fn seal(paths: &ProjectPaths) -> Checked<std::path::PathBuf> {
    let directory = paths
        .metadata_file()
        .parent()
        .required_because("the metadata lives in a directory")?
        .to_path_buf();
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500))
        .required_because("take away the write permission")?;
    Ok(directory)
}

fn unseal(directory: &std::path::Path) -> Checked {
    std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700))
        .required_because("give it back so the temporary directory can be removed")?;
    Ok(())
}

#[test]
fn a_cleanup_that_fails_before_the_commit_point_keeps_the_project_managed() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = no_secrets(clean_host(&fixture, &project)?, project.sandbox.as_str());
    expect_successful_removal(&host);

    let mut prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .required_because("prepare")?;

    let sealed = seal(&project.paths)?;
    let error = destroy(&host, &mut prepared).refused_because("the metadata cannot be removed")?;
    assert_eq!(error.first_id(), Some(ErrorId::CleanupFailed));
    assert!(
        project.paths.metadata_file().exists(),
        "the project is still managed, so destroy can be run again"
    );
    unseal(&sealed)?;
    Ok(())
}

/// 診断が示したpath。
fn reported_path(error: &Error) -> Option<String> {
    error.diagnostics()[0]
        .facts
        .iter()
        .find_map(|fact| match fact {
            crate::design::Fact::OneLine { label, value }
                if label.id == "diagnostic-path-label" =>
            {
                Some(value.as_str().to_string())
            }
            _ => None,
        })
}

#[test]
fn a_cache_that_cannot_be_removed_names_the_cache_and_keeps_the_project_managed() -> Checked {
    // 後片付けの失敗はcommit pointの手前で起こる。どのpathが残ったかを示さなければ、
    // 利用者はやり直す前に何を直せばよいか分からない。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::create_dir_all(project.paths.cache_dir()).required()?;
    let host = no_secrets(clean_host(&fixture, &project)?, project.sandbox.as_str());
    expect_successful_removal(&host);

    let mut prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .required_because("prepare")?;

    let sealed = seal(&project.paths)?;
    let error =
        destroy(&host, &mut prepared).refused_because("the cache directory cannot be removed")?;
    assert_eq!(error.first_id(), Some(ErrorId::CleanupFailed));
    assert_eq!(
        reported_path(&error),
        Some(paths::display(&project.paths.cache_dir())),
        "the diagnostic names the path that is still there"
    );
    assert!(
        project.paths.metadata_file().exists(),
        "the project stays managed so destroy can be run again"
    );
    unseal(&sealed)?;
    Ok(())
}

#[test]
fn a_metadata_file_that_is_a_symlink_is_not_followed_and_the_project_stays_managed() -> Checked {
    // 管理解除のcommit pointであっても、symlinkの先は消さない。link自体を消して
    // 管理を解いたことにもしない。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = no_secrets(clean_host(&fixture, &project)?, project.sandbox.as_str());
    expect_successful_removal(&host);

    let mut prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .required_because("prepare")?;

    // metadataを読んだあとで、その位置がほかのfileへのlinkへ差し替えられた。
    let elsewhere = fixture.dir.path().join("elsewhere.yaml");
    std::fs::write(&elsewhere, b"not ours\n").required()?;
    std::fs::remove_file(project.paths.metadata_file()).required()?;
    std::os::unix::fs::symlink(&elsewhere, project.paths.metadata_file()).required()?;

    let error =
        destroy(&host, &mut prepared).refused_because("a symlinked metadata path is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathSymlink));
    assert!(
        elsewhere.exists(),
        "what the link pointed at is left where it was"
    );
    assert!(
        project.paths.lock_file().exists(),
        "the run stops before it clears the lock file"
    );
    Ok(())
}

#[test]
fn a_sandbox_that_appears_after_the_plan_was_made_is_not_left_behind_silently() -> Checked {
    // 計画時に不在だったからといって、削除commandを省いた実行を成功にはできない。
    // confirmationは不在という観測へ結び付いており、remove直前の再評価でSandboxが
    // 現れていれば、fingerprintの不一致として拒否する。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let running = format!(
        r#"{{"sandboxes":[{}]}}"#,
        fixture.entry(&project, "running")?
    );
    let host = no_secrets(clean_host(&fixture, &project)?, project.sandbox.as_str());
    // 一覧は末尾から取り出される。prepareの観測では不在だが、remove直前の再評価では
    // 現れている状況を作る。それ以外の観測(worktree一覧やgit状態)は`clean_host`が
    // 既に揃えたものをそのまま使う。
    *host.listing.borrow_mut() = vec![running, r#"{"sandboxes":[]}"#.to_string()];

    let mut prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .required_because("prepare")?;
    assert_eq!(prepared.plan.state, ProjectState::NotCreated);

    let error = destroy(&host, &mut prepared).refused_because("the sandbox is listed after all")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProtectionStateChanged));
    assert!(
        !host.ran("rm "),
        "the plan said there was nothing to remove: {:?}",
        host.calls()
    );
    assert!(
        project.paths.metadata_file().exists(),
        "the project stays managed so destroy can be run again"
    );
    Ok(())
}

#[test]
fn a_lock_file_left_behind_is_a_warning_because_the_project_is_already_unmanaged() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = no_secrets(clean_host(&fixture, &project)?, project.sandbox.as_str());
    expect_successful_removal(&host);

    let mut prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .required_because("prepare")?;

    // metadataは消えており、lock fileだけが残せない状態を作る。
    std::fs::remove_file(project.paths.metadata_file()).required()?;
    let sealed = seal(&project.paths)?;

    let outcome =
        destroy(&host, &mut prepared).required_because("the project is unmanaged already")?;
    assert_eq!(outcome.warnings.len(), 1, "the leftover is reported once");
    assert!(
        project.paths.lock_file().exists(),
        "the lock file is the thing that was left"
    );
    unseal(&sealed)?;
    Ok(())
}

#[test]
fn a_sandbox_that_survives_its_removal_keeps_the_management_data() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    // 削除後の一覧にも対象が残り続ける。
    let host = clean_host(&fixture, &project)?;

    let mut prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .required_because("prepare")?;

    let error = destroy(&host, &mut prepared).refused_because("the sandbox is still there")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxStillPresent));
    assert!(
        !host.ran("secret rm"),
        "a sandbox that still exists keeps the token it was given"
    );
    assert!(
        project.paths.metadata_file().exists(),
        "the project stays managed so destroy can be run again"
    );
    Ok(())
}

/// 管理解除をcommitした案件として、registryから外す要求を組み立てる。
fn unregistration_of(project: &Registered) -> Unregistration {
    Unregistration {
        paths: project.paths.clone(),
        repository: project.metadata.repository.clone(),
    }
}

#[test]
fn unregistering_removes_the_entry_of_that_project_and_no_other() -> Checked {
    let fixture = Fixture::new()?;
    let target = fixture.register("example-org/example-repo")?;
    let other = fixture.register("other-org/other-repo")?;
    // metadataの削除が管理解除のcommit pointである。
    std::fs::remove_file(target.paths.metadata_file()).required_because("remove the metadata")?;

    assert!(
        unregister(&fixture.location, &unregistration_of(&target))
            .required_because("unregister")?
            .is_none()
    );

    let registry =
        crate::registry::load(&fixture.location).required_because("the registry stays valid")?;
    assert_eq!(
        registry
            .entries()
            .iter()
            .map(|entry| entry.canonical_id().as_str())
            .collect::<Vec<_>>(),
        vec!["other-org/other-repo"]
    );
    assert!(
        other.paths.metadata_file().exists(),
        "another project is untouched"
    );
    // 利用者の成果物は残す。registryから外れたあとは未登録のhost artifactとして扱う。
    assert!(target.paths.root().is_dir());
    Ok(())
}

#[test]
fn an_entry_that_is_already_gone_is_not_an_error_to_remove_again() -> Checked {
    // registry entryの削除まで終えた直後に中断した実行は、同じcommandでやり直される。
    // 外すものが無い状態は、失敗ではなく何もしないことである。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let unregistration = unregistration_of(&project);
    std::fs::remove_file(project.paths.metadata_file()).required_because("remove the metadata")?;
    let mut guard =
        RegistryGuard::acquire(&fixture.location).required_because("acquire the registry lock")?;
    guard
        .remove(project.metadata.canonical_id())
        .required_because("remove the entry")?;
    drop(guard);

    assert!(
        unregister(&fixture.location, &unregistration)
            .required_because("an entry that is not there needs no removal")?
            .is_none(),
        "nothing is reported for an entry that was already removed"
    );
    let registry =
        crate::registry::load(&fixture.location).required_because("the registry stays valid")?;
    assert!(registry.entries().is_empty());
    Ok(())
}

#[test]
fn a_project_root_that_is_gone_entirely_loses_its_entry() -> Checked {
    // rootごと消えていれば、その場所へ登録し直された案件はない。observeできない状態
    // ではなく、管理が解かれたことを確かめられた状態である。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let unregistration = unregistration_of(&project);
    std::fs::remove_dir_all(project.paths.root()).required_because("remove the project root")?;

    assert!(
        unregister(&fixture.location, &unregistration)
            .required_because("unregister")?
            .is_none(),
        "a root that is gone is not reported as a re-registration"
    );
    let registry =
        crate::registry::load(&fixture.location).required_because("the registry stays valid")?;
    assert!(
        registry.find(project.metadata.canonical_id()).is_none(),
        "the entry of a project that no longer exists is removed"
    );
    Ok(())
}

#[test]
fn a_root_whose_place_cannot_even_be_looked_at_keeps_the_entry() -> Checked {
    // rootの不在とrootを見られないことは違う。読めなかった実行は、entryを消して
    // 案件を発見不能にするより、読めなかったことを述べて止まる。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let unregistration = unregistration_of(&project);
    let parent = project
        .paths
        .root()
        .parent()
        .required_because("the project root has a parent")?
        .to_path_buf();
    std::fs::remove_dir_all(project.paths.root()).required_because("remove the project root")?;
    std::fs::remove_dir(&parent).required_because("remove the parent directory")?;
    // 親がdirectoryでなくなり、rootの位置はもう辿れない。
    std::fs::write(&parent, b"not a directory\n").required_because("put a file in its place")?;

    let error = unregister(&fixture.location, &unregistration)
        .refused_because("the place the root would be cannot be read")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnreadable));
    let registry =
        crate::registry::load(&fixture.location).required_because("the registry stays valid")?;
    assert!(
        registry.find(project.metadata.canonical_id()).is_some(),
        "the entry stays until the state can be observed"
    );
    Ok(())
}

#[test]
fn a_project_registered_again_during_the_removal_keeps_its_entry() -> Checked {
    // destroyはmetadataを消してproject lockを手放したあとでregistry lockを取る。その
    // 隙間に同じ案件のaddが登録をやり直していれば、entryは新しい登録意図を指している。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let host = no_secrets(clean_host(&fixture, &project)?, project.sandbox.as_str());
    expect_successful_removal(&host);
    let mut prepared = prepare(
        &fixture.location,
        Some(&project_id("example-org/example-repo")?),
        false,
        &host,
        &mut ScriptedPrompt::choosing(0),
        &fixture.workspace_root,
    )
    .required_because("prepare")?;

    let unregistration = prepared.unregistration();
    destroy(&host, &mut prepared).required_because("destroy")?;
    drop(prepared);

    // project lockが空いているあいだに、別の実行のaddがmetadataを作り直した。
    metadata::create(&project.paths, &project.metadata)
        .required_because("register the project again")?;

    let warning = unregister(&fixture.location, &unregistration)
        .required_because("unregister")?
        .required_because("the re-registration is reported")?;
    assert_eq!(warning.description.id, "warning-project-registered-again");
    let registry =
        crate::registry::load(&fixture.location).required_because("the registry stays valid")?;
    assert!(
        registry.find(project.metadata.canonical_id()).is_some(),
        "the project that was registered again stays discoverable"
    );
    Ok(())
}

#[test]
fn an_entry_that_names_another_root_is_left_to_the_run_that_recorded_it() -> Checked {
    // 同じcanonical project IDでも、別のrootを指すentryはこの実行が記録したものではない。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let unregistration = unregistration_of(&project);
    std::fs::remove_file(project.paths.metadata_file()).required_because("remove the metadata")?;

    // 別の実行が、entryを外して別のrootへ登録し直した。
    let mut guard =
        RegistryGuard::acquire(&fixture.location).required_because("acquire the registry lock")?;
    guard
        .remove(project.metadata.canonical_id())
        .required_because("remove the entry")?;
    drop(guard);
    let elsewhere = fixture.dir.path().join("elsewhere.project");
    std::fs::create_dir_all(&elsewhere).required_because("create the other root")?;
    fixture.record(&elsewhere, project.metadata.repository.clone())?;

    let warning = unregister(&fixture.location, &unregistration)
        .required_because("unregister")?
        .required_because("an entry another run recorded is reported")?;
    assert_eq!(warning.description.id, "warning-project-registered-again");
    let registry =
        crate::registry::load(&fixture.location).required_because("the registry stays valid")?;
    assert_eq!(
        registry
            .find(project.metadata.canonical_id())
            .required_because("the entry stays")?
            .project_root(),
        elsewhere,
        "the registration of the other run is untouched"
    );
    Ok(())
}

#[test]
fn a_root_that_cannot_be_verified_keeps_the_entry_instead_of_guessing() -> Checked {
    // 再登録の有無を観測できなければ、管理解除が維持されているとみなさない。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    let unregistration = unregistration_of(&project);
    std::fs::remove_dir_all(project.paths.root()).required_because("remove the project root")?;
    std::fs::write(project.paths.root(), b"not a project root\n")
        .required_because("put a file in its place")?;

    let error = unregister(&fixture.location, &unregistration)
        .refused_because("a root that is not a directory cannot be verified")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathUnexpectedType));
    let registry =
        crate::registry::load(&fixture.location).required_because("the registry stays valid")?;
    assert!(
        registry.find(project.metadata.canonical_id()).is_some(),
        "the entry stays until the state can be observed"
    );
    Ok(())
}

#[test]
fn an_entry_left_behind_by_a_crash_after_the_commit_point_is_tolerated() -> Checked {
    // metadata削除後、registry entry削除前に中断すると、不整合entryが残り得る。
    // 通常modeで推測して消さず、read-onlyの観測へ残す。
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::remove_file(project.paths.metadata_file()).required_because("remove the metadata")?;

    let registry =
        crate::registry::load(&fixture.location).required_because("the registry stays valid")?;
    assert_eq!(registry.entries().len(), 1);

    let error = select::find(&fixture.location, &project_id("example-org/example-repo")?)
        .and_then(select::Candidate::lock)
        .refused_because("the project cannot be worked on")?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectIncomplete));
    Ok(())
}
