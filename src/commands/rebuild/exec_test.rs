use crate::cli::Interactivity;
use crate::command::HostEnvironment;
use crate::commands::Context;
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::design::{OutputPolicy, PromptUi, Ui};
use crate::diagnostics::ExitCode;
use crate::hash::sha256_hex;
use crate::i18n::Locale;
use crate::metadata;
use crate::support::image;

use crate::testing::host::FakeSbx;
use crate::testing::image::template_listing;
use crate::testing::outcome::{Checked, Required};
use crate::testing::project::{Fixture, Registered, project_id};
use crate::testing::protection::clean_host;

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
    let policy = OutputPolicy::plain();
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let code = {
        let mut ui = Ui::capture(Locale::En, policy, &mut stdout, &mut stderr);
        let mut prompt = PromptUi::new(
            Locale::En,
            policy.stderr,
            Box::new(ScriptedKeys::typing(typed)),
            Box::new(RecordedScreen::new()),
        );
        let context = Context {
            location: &fixture.location,
            workspace_root: &fixture.workspace_root,
            lang: Some(Locale::En),
            interactivity: Interactivity {
                stdin_is_tty: true,
                stderr_is_tty: true,
            },
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
