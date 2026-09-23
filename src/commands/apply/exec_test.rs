use std::path::Path;

use crate::commands::Context;
use crate::config::ConfigLocation;
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::design::{PromptUi, RenderingPolicy, Ui};
use crate::diagnostics::ExitCode;
use crate::i18n::Locale;
use crate::metadata::RebuildIntent;

use crate::testing::outcome::{Checked, Required};
use crate::testing::value::DIGEST;

use super::super::fake::*;
use super::*;

/// `apply --files --all`を入口から実行し、terminalへ出るはずのものを受け取る。
fn exec_all(
    location: &ConfigLocation,
    workspace_root: &Path,
    host: &FakeSbx,
) -> Checked<(ExitCode, String, String)> {
    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    let policy = RenderingPolicy::plain();
    let code = {
        let mut ui = Ui::capture(Locale::En, policy, &mut stdout, &mut stderr);
        let mut prompt = PromptUi::new(
            Locale::En,
            policy.stderr,
            Box::new(ScriptedKeys::confirming()),
            Box::new(RecordedScreen::new()),
        );
        let context = Context {
            location,
            workspace_root,
            locale: Locale::En,
            can_prompt: false,
        };
        exec(
            &Args {
                project: None,
                all: true,
                files: true,
                force: false,
                worktrees: None,
            },
            &context,
            &mut ui,
            host,
            &mut prompt,
        )
    };
    Ok((
        code,
        String::from_utf8(stdout).required_because("UTF-8")?,
        String::from_utf8(stderr).required_because("UTF-8")?,
    ))
}

#[test]
fn applying_to_every_project_prints_one_row_per_project() -> Checked {
    let (_home, location, parent, _config, workspace_root) = setup(Vec::new())?;
    write_metadata(&location, &parent, None)?;
    let host = FakeSbx::listing(&listing(&workspace_root, "running")?);

    let (code, stdout, stderr) = exec_all(&location, &workspace_root, &host)?;

    assert_eq!(code, ExitCode::Success, "{stderr}");
    let row = stdout
        .lines()
        .find(|line| line.contains("Example-Org/Example-Repo"))
        .required_because("the project has a row")?;
    // configが宣言するfileは無い。置くものが無い案件は変わらない。
    assert!(row.contains("unchanged"), "{stdout}");
    Ok(())
}

#[test]
fn a_project_that_could_not_be_applied_fails_the_run() -> Checked {
    let (_home, location, parent, _config, workspace_root) = setup(Vec::new())?;
    write_metadata(
        &location,
        &parent,
        Some(RebuildIntent {
            target_dockerfile_sha256: DIGEST.into(),
            previous_dockerfile_sha256: DIGEST.into(),
        }),
    )?;
    let host = FakeSbx::listing(&listing(&workspace_root, "running")?);

    let (code, stdout, stderr) = exec_all(&location, &workspace_root, &host)?;

    assert_eq!(code, ExitCode::Failure);
    assert!(stdout.contains("failed"), "{stdout}");
    assert!(stderr.contains("rebuild-intent-pending"), "{stderr}");
    Ok(())
}

#[test]
fn nothing_registered_is_reported_as_an_error() -> Checked {
    let (_home, location, _parent, _config, workspace_root) = setup(Vec::new())?;
    let host = FakeSbx::listing(r#"{"sandboxes":[]}"#);

    let (code, _stdout, stderr) = exec_all(&location, &workspace_root, &host)?;

    assert_eq!(code, ExitCode::Failure);
    assert!(stderr.contains("no-managed-projects"), "{stderr}");
    Ok(())
}
