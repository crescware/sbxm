use crate::boundary::host::HostEnvironment;
use crate::commands::Context;
use crate::design::prompt::{Key, RecordedScreen, ScriptedKeys};
use crate::design::{PromptUi, RenderingPolicy, Ui};
use crate::diagnostics::ExitCode;
use crate::hash::sha256_hex;
use crate::i18n::Locale;
use crate::metadata;
use crate::support::image;

use crate::testing::host::FakeSbx;
use crate::testing::image::template_listing;
use crate::testing::outcome::{Checked, Required};
use crate::testing::project::{Fixture, Registered, project_id};
use crate::testing::protection::{clean_host, commit_only_in_the_sandbox};

/// `exec`が書いたstdoutとstderr、そして終了statusを取り出す。
///
/// `exec`は`Context`が運ぶworkspace rootを使うため、ここではfixtureのrootを渡す。
/// `run`のtestが通らない経路、つまり計画、確認、実行のつなぎ目だけを確かめる。
struct Ran {
    code: ExitCode,
    stdout: String,
    stderr: String,
}

fn run(fixture: &Fixture, host: &dyn HostEnvironment, typed: &str) -> Checked<Ran> {
    run_with(fixture, host, ScriptedKeys::typing(typed))
}

/// 打鍵をそのまま並べて`exec`を通す。
fn run_pressing(fixture: &Fixture, host: &dyn HostEnvironment, keys: &[Key]) -> Checked<Ran> {
    run_with(fixture, host, ScriptedKeys::pressing(keys))
}

fn run_with(fixture: &Fixture, host: &dyn HostEnvironment, keys: ScriptedKeys) -> Checked<Ran> {
    let policy = RenderingPolicy::plain();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = {
        let mut ui = Ui::capture(Locale::En, policy, &mut stdout, &mut stderr);
        let mut prompt = PromptUi::new(
            Locale::En,
            policy.stderr,
            Box::new(keys),
            Box::new(RecordedScreen::new()),
        );
        let context = Context {
            location: &fixture.location,
            workspace_root: &fixture.workspace_root,
            locale: Locale::En,
            can_prompt: true,
        };
        super::exec(
            Some(&project_id("example-org/example-repo")?),
            &context,
            &mut ui,
            host,
            &mut prompt,
        )
    };
    Ok(Ran {
        code,
        stdout: String::from_utf8(stdout).required_because("rebuild stdout is UTF-8")?,
        stderr: String::from_utf8(stderr).required_because("rebuild stderr is UTF-8")?,
    })
}

/// 適用済みのDockerfileと、その世代のimageとtemplateを既に持つhost。
///
/// buildもtemplate loadも起こらないため、`exec`が選ぶ経路だけがtestの対象になる。
fn host_with_the_applied_generation(
    fixture: &Fixture,
    project: &mut Registered,
) -> Checked<FakeSbx> {
    std::fs::write(project.paths.dockerfile(), "unchanged\n").required()?;
    let target = sha256_hex(b"unchanged\n");
    project.metadata.provisioning.dockerfile_sha256 = target.clone();
    metadata::update(&project.paths, &project.metadata).required()?;

    let image = image::image_name(&project.sandbox, &target);
    Ok(clean_host(fixture, project)?
        .answering(&format!("image ls --quiet {image}"), 0, "sha256:existing\n")
        .answering(
            &format!("image inspect {image}"),
            0,
            &format!(
                r#"[{{"Id":"sha256:existing","Config":{{"Labels":{{"io.crescware.sbxm.canonical-id":"example-org/example-repo","io.crescware.sbxm.dockerfile-sha256":"{target}","io.crescware.sbxm.metadata-version":"1"}}}}}}]"#
            ),
        )
        .answering("template ls --json", 0, &template_listing(&image)?))
}

/// `exec`が観測するworkspaceを申告する、稼働中のSandbox 1件の一覧。
fn running(fixture: &Fixture, project: &Registered) -> Checked<String> {
    Ok(format!(
        r#"{{"sandboxes":[{}]}}"#,
        fixture.entry(project, "running")?
    ))
}

#[test]
fn a_sandbox_that_disappeared_after_it_was_confirmed_is_reported_instead_of_rebuilt() -> Checked {
    // 計画を見せ、Sandbox名の完全一致で確認を取り、実行が拒否されるまでを`exec`ごと通す。
    // 確認の直後に対象Sandboxが手作業で消えていれば、`exec`は作り直さず理由を述べる。
    let fixture = Fixture::new()?;
    let mut project = fixture.register("example-org/example-repo")?;
    let host = host_with_the_applied_generation(&fixture, &mut project)?;

    // 一覧は末尾から取り出される。計画と確認までは稼働中のSandboxを観測し、実行が
    // 状態を取り直す時点では消えている。
    *host.listing.borrow_mut() = vec![
        r#"{"sandboxes":[]}"#.to_string(),
        running(&fixture, &project)?,
        running(&fixture, &project)?,
    ];

    let ran = run(&fixture, &host, project.sandbox.as_str())?;

    assert_eq!(ran.code, ExitCode::Failure, "{}{}", ran.stdout, ran.stderr);
    // 確認の前に、何を失うかを述べた計画が出ている。
    assert!(
        ran.stdout.contains(project.sandbox.as_str()),
        "the plan names the sandbox it would recreate: {}",
        ran.stdout
    );
    // 拒否は理由ごと述べる。黙って作り直しへ進まない。
    assert!(
        ran.stderr.contains("changed"),
        "the refusal says the state changed: {}",
        ran.stderr
    );
    assert!(
        !host.ran("rm ") && !host.ran("create --name"),
        "nothing is removed or created once the state has changed: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_name_that_does_not_match_the_sandbox_stops_before_anything_is_touched() -> Checked {
    // 確認は完全一致だけを合図にする。違う綴りでは実行へ進まない。
    let fixture = Fixture::new()?;
    let mut project = fixture.register("example-org/example-repo")?;
    let host = host_with_the_applied_generation(&fixture, &mut project)?;
    *host.listing.borrow_mut() = vec![running(&fixture, &project)?, running(&fixture, &project)?];

    let ran = run(&fixture, &host, "not-the-sandbox-name")?;

    assert_eq!(ran.code, ExitCode::Failure, "{}{}", ran.stdout, ran.stderr);
    assert!(
        !host.ran("rm ") && !host.ran("create --name"),
        "a name that does not match touches nothing: {:?}",
        host.calls()
    );
    let stored = metadata::load(&project.paths)
        .required()?
        .required_because("the project is still managed")?;
    assert!(
        stored.rebuild.is_none(),
        "an unconfirmed rebuild commits no intent"
    );
    Ok(())
}

#[test]
fn a_project_that_is_not_managed_is_reported_before_the_plan_is_drawn() -> Checked {
    // 対象が決まらなければ計画も確認も無い。`exec`は最初の失敗をそのまま述べる。
    let fixture = Fixture::new()?;
    let host = FakeSbx::listing(r#"{"sandboxes":[]}"#);

    let ran = run(&fixture, &host, "")?;

    assert_eq!(ran.code, ExitCode::Failure, "{}{}", ran.stdout, ran.stderr);
    assert!(
        ran.stdout.is_empty(),
        "no plan is drawn for a project that is not managed: {}",
        ran.stdout
    );
    // 案件が決まらない以上、Sandboxの中も覗かない。
    assert!(
        !host.ran("rm ") && !host.ran("create --name") && !host.ran("exec "),
        "{:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_commit_saved_to_the_host_on_request_lets_the_plan_be_drawn() -> Checked {
    // 保護の検査が止めたあと、hostへ保存する選択をすれば、同じ案件をもう一度準備して
    // 計画と確認へ進む。確認はcancelし、作り直しには進ませない。
    let fixture = Fixture::new()?;
    let mut project = fixture.register("example-org/example-repo")?;
    let host = commit_only_in_the_sandbox(
        host_with_the_applied_generation(&fixture, &mut project)?,
        &project,
    );
    *host.listing.borrow_mut() = vec![running(&fixture, &project)?];

    let ran = run_pressing(&fixture, &host, &[Key::Enter, Key::Escape])?;

    assert_eq!(ran.code, ExitCode::Canceled, "{}{}", ran.stdout, ran.stderr);
    assert!(
        ran.stderr.contains("origin-commit-unreachable"),
        "the refusal is shown before the offer: {}",
        ran.stderr
    );
    assert!(
        host.ran("bundle create"),
        "the commits are saved to the host: {:?}",
        host.calls()
    );
    assert!(
        ran.stdout.contains("Target generation"),
        "the plan is drawn once the commits are saved: {}",
        ran.stdout
    );
    assert!(
        !host.ran("rm ") && !host.ran("create --name"),
        "{:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_refusal_that_remains_after_saving_is_reported_without_asking_again() -> Checked {
    // 保存しても届かないcommitが残るなら、同じ問いを繰り返さずに断る。打鍵は1問分しか
    // 用意しない。2度目を訊けば、打鍵が尽きた失敗として現れる。
    let fixture = Fixture::new()?;
    let mut project = fixture.register("example-org/example-repo")?;
    let host = commit_only_in_the_sandbox(
        host_with_the_applied_generation(&fixture, &mut project)?,
        &project,
    )
    .answering_in_turn(
        &format!(
            "for-each-ref --format=%(refname) %(objectname) refs/sbx/{}/",
            project.sandbox.as_str()
        ),
        &[(0, "")],
    );
    *host.listing.borrow_mut() = vec![running(&fixture, &project)?];

    let ran = run_pressing(&fixture, &host, &[Key::Enter])?;

    assert_eq!(ran.code, ExitCode::Failure, "{}{}", ran.stdout, ran.stderr);
    assert_eq!(
        ran.stderr.matches("origin-commit-unreachable").count(),
        2,
        "{}",
        ran.stderr
    );
    assert!(!ran.stderr.contains("prompt-unreadable"), "{}", ran.stderr);
    assert!(
        !ran.stdout.contains("Target generation"),
        "no plan is drawn while the refusal remains: {}",
        ran.stdout
    );
    Ok(())
}
