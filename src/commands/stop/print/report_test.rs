use crate::commands::stop::{StopOutcome, StopResult};
use crate::design::OutputPolicy;
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::i18n::Locale;

use crate::testing::outcome::{Checked, Required};

use super::*;

/// 2つのstreamを別々に受け取る。結果と診断の行き先が入れ替わっても気付けるようにする。
struct Printed {
    code: ExitCode,
    stdout: String,
    stderr: String,
}

fn print(stopped: &StopReport) -> Checked<Printed> {
    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    let code = {
        let mut ui = Ui::capture(Locale::En, OutputPolicy::plain(), &mut stdout, &mut stderr);
        report(&mut ui, stopped)
    };
    Ok(Printed {
        code,
        stdout: String::from_utf8(stdout).required_because("UTF-8")?,
        stderr: String::from_utf8(stderr).required_because("UTF-8")?,
    })
}

fn outcome(project: &str, sandbox: &str, result: StopResult) -> StopOutcome {
    StopOutcome {
        project: project.to_owned(),
        sandbox: sandbox.to_owned(),
        result,
    }
}

fn still_running(sandbox: &str) -> Diagnostic {
    Diagnostic::new(
        ErrorId::SandboxStillRunning,
        crate::msg!("error-sandbox-still-running", sandbox = sandbox),
    )
}

fn row<'a>(output: &'a str, project: &str) -> Checked<&'a str> {
    output
        .lines()
        .find(|line| line.contains(project))
        .required_because("the project row is printed")
}

#[test]
fn a_run_without_failures_succeeds_and_leaves_stderr_untouched() -> Checked {
    let stopped = StopReport {
        outcomes: vec![
            outcome("owner/alpha", "sbxm-owner-alpha", StopResult::Stopped),
            outcome("owner/bravo", "sbxm-owner-bravo", StopResult::Unchanged),
        ],
        failures: Vec::new(),
    };

    let printed = print(&stopped)?;

    assert_eq!(printed.code, ExitCode::Success);
    // 失敗が1件も無ければ、診断のblockは空のまま書かれるため空行も残らない。
    assert!(printed.stderr.is_empty(), "{:?}", printed.stderr);
    Ok(())
}

#[test]
fn every_target_reaches_stdout_with_the_result_it_was_given() -> Checked {
    let stopped = StopReport {
        outcomes: vec![
            outcome("owner/alpha", "sbxm-owner-alpha", StopResult::Stopped),
            outcome("owner/bravo", "sbxm-owner-bravo", StopResult::Unchanged),
        ],
        failures: Vec::new(),
    };

    let printed = print(&stopped)?;

    let alpha = row(&printed.stdout, "owner/alpha")?;
    assert!(alpha.contains("sbxm-owner-alpha"), "{alpha:?}");
    assert!(alpha.contains(StopResult::Stopped.as_str()), "{alpha:?}");
    assert!(!alpha.contains(StopResult::Unchanged.as_str()), "{alpha:?}");

    let bravo = row(&printed.stdout, "owner/bravo")?;
    assert!(bravo.contains("sbxm-owner-bravo"), "{bravo:?}");
    assert!(bravo.contains(StopResult::Unchanged.as_str()), "{bravo:?}");
    assert!(!bravo.contains(StopResult::Stopped.as_str()), "{bravo:?}");
    Ok(())
}

#[test]
fn one_failed_target_decides_the_exit_code() -> Checked {
    let stopped = StopReport {
        outcomes: vec![
            outcome("owner/alpha", "sbxm-owner-alpha", StopResult::Failed),
            outcome("owner/bravo", "sbxm-owner-bravo", StopResult::Unchanged),
        ],
        failures: vec![still_running("sbxm-owner-alpha")],
    };

    let printed = print(&stopped)?;

    assert_eq!(printed.code, ExitCode::Failure);
    Ok(())
}

#[test]
fn a_failure_is_reported_without_hiding_the_results() -> Checked {
    let stopped = StopReport {
        outcomes: vec![outcome(
            "owner/alpha",
            "sbxm-owner-alpha",
            StopResult::Failed,
        )],
        failures: vec![still_running("sbxm-owner-alpha")],
    };

    let printed = print(&stopped)?;

    // 表そのものが結論であるため、失敗した対象も結果の表に残る。
    assert!(
        printed.stdout.contains(StopResult::Failed.as_str()),
        "{:?}",
        printed.stdout
    );
    // 結果表はstdoutだけにあり、診断がstdoutへ混ざらない。
    assert!(!printed.stdout.contains("error:"), "{:?}", printed.stdout);
    assert!(
        printed.stderr.contains("sbxm-owner-alpha"),
        "{:?}",
        printed.stderr
    );
    // 診断はstderrだけにあり、結果表がstderrへ混ざらない。
    assert!(
        !printed.stderr.contains("owner/alpha"),
        "{:?}",
        printed.stderr
    );
    Ok(())
}

#[test]
fn every_failure_is_written_as_its_own_diagnostic() -> Checked {
    let stopped = StopReport {
        outcomes: vec![
            outcome("owner/alpha", "sbxm-owner-alpha", StopResult::Failed),
            outcome("owner/bravo", "sbxm-owner-bravo", StopResult::Unchanged),
        ],
        failures: vec![
            still_running("sbxm-owner-alpha"),
            still_running("sbxm-owner-bravo"),
        ],
    };

    let printed = print(&stopped)?;

    assert_eq!(
        printed.stderr.matches("error:").count(),
        2,
        "{:?}",
        printed.stderr
    );
    Ok(())
}
