use std::os::unix::fs::PermissionsExt;

use crate::cli::Interactivity;
use crate::commands::Context;
use crate::commands::rebuild::fake::ready_to_switch;
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::design::{OutputPolicy, PromptUi, Ui};
use crate::diagnostics::ExitCode;
use crate::hash::sha256_hex;
use crate::i18n::Locale;
use crate::metadata;
use crate::project::{ProjectId, SandboxLayout};
use crate::support::image;

use crate::testing::image::template_listing;
use crate::testing::outcome::{Checked, Required};
use crate::testing::project::{Fixture, project_id};
use crate::testing::protection::clean_host;

/// `exec`を1回起動し、終了codeとstdoutへ書かれた内容を返す。`typed`は確認promptへ
/// そのまま打ち込む文字列。
fn run_exec(
    fixture: &Fixture,
    host: &dyn crate::command::HostEnvironment,
    project: Option<&ProjectId>,
    typed: &str,
) -> Checked<(ExitCode, String)> {
    let context = Context {
        location: &fixture.location,
        workspace_root: &fixture.workspace_root,
        lang: Some(Locale::En),
        interactivity: Interactivity {
            stdin_is_tty: true,
            stderr_is_tty: true,
        },
    };
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
        super::exec(project, &context, &mut ui, host, &mut prompt)
    };
    let printed = String::from_utf8(stdout).required_because("stdout is valid UTF-8")?;
    Ok((code, printed))
}

#[test]
fn a_dockerfile_that_did_not_change_still_recreates_the_sandbox_via_exec() -> Checked {
    let fixture = Fixture::new()?;
    let mut project = fixture.register("example-org/example-repo")?;
    // 適用済みhashと同じ内容のDockerfileを置く。
    std::fs::write(project.paths.dockerfile(), "unchanged\n").required()?;
    let target = sha256_hex(b"unchanged\n");
    project.metadata.provisioning.dockerfile_sha256 = target.clone();
    metadata::update(&project.paths, &project.metadata).required()?;

    let image = image::image_name(&project.sandbox, &target);
    let workspace = fixture.workspace_root.join(project.sandbox.as_str());
    std::fs::create_dir_all(&workspace).required()?;
    std::fs::set_permissions(&workspace, std::fs::Permissions::from_mode(0o700)).required()?;

    let host = clean_host(&fixture, &project)?
        .answering(&format!("image ls --quiet {image}"), 0, "sha256:existing\n")
        .answering(
            &format!("image inspect {image}"),
            0,
            &format!(
                r#"[{{"Id":"sha256:existing","Config":{{"Labels":{{"io.crescware.sbxm.canonical-id":"example-org/example-repo","io.crescware.sbxm.dockerfile-sha256":"{target}","io.crescware.sbxm.metadata-version":"1"}}}}}}]"#
            ),
        )
        .answering("template ls --json", 0, &template_listing(&image)?);

    let layout = SandboxLayout::new(project.metadata.canonical_id());
    let git_dir = layout.bare_git_dir();
    let worktree = layout.worktree(0);
    let name = project.sandbox.as_str();
    let host = ready_to_switch(host, name, &git_dir, &worktree);

    let running = format!(
        r#"{{"sandboxes":[{}]}}"#,
        fixture.entry(&project, "running")?
    );
    let created = format!(
        r#"{{"sandboxes":[{{"name":"{}","status":"running","workspaces":["{}"]}}]}}"#,
        project.sandbox,
        workspace.display()
    );
    // 一覧は末尾から取り出される。状態の判定、削除前の再評価、削除完了の確認、作成前の
    // 確認までは稼働中のSandboxが対象と観測し、作成後は新しいSandboxを観測する。
    *host.listing.borrow_mut() = vec![
        created,
        r#"{"sandboxes":[]}"#.to_string(),
        r#"{"sandboxes":[]}"#.to_string(),
        running.clone(),
        running,
    ];

    let project_id = project_id("example-org/example-repo")?;
    let (code, printed) = run_exec(&fixture, &host, Some(&project_id), name)?;

    assert_eq!(code, ExitCode::Success, "{printed}");
    assert!(
        !host.ran("build"),
        "the existing image is reused: {:?}",
        host.calls()
    );
    assert!(
        host.ran("rm ") && host.ran("create --name"),
        "an unchanged Dockerfile still recreates the sandbox: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_mistyped_sandbox_name_stops_the_rebuild_before_anything_is_touched() -> Checked {
    let fixture = Fixture::new()?;
    let project = fixture.register("example-org/example-repo")?;
    std::fs::write(project.paths.dockerfile(), "FROM scratch\n").required()?;
    let host = clean_host(&fixture, &project)?;

    let project_id = project_id("example-org/example-repo")?;
    let (code, printed) = run_exec(&fixture, &host, Some(&project_id), "yes")?;

    assert_ne!(code, ExitCode::Success, "{printed}");
    assert!(
        !host.ran("rm ") && !host.ran("create --name"),
        "nothing is touched without the exact sandbox name: {:?}",
        host.calls()
    );
    Ok(())
}
