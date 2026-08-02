//! global scopeの`status`の出力。
//!
//! 表と診断は行き先が別であり、入れ替わってもexit codeでは気付けない。2つのstreamを
//! 別々に受け取り、どちらに何が出たかを確かめる。

use crate::design::{OutputPolicy, Warning};
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::i18n::Locale;
use crate::support::{Row, StatusValue};

use crate::testing::outcome::{Checked, Required};

use super::*;

struct Printed {
    code: ExitCode,
    stdout: String,
    stderr: String,
}

fn print(status: &GlobalStatus) -> Checked<Printed> {
    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    let code = {
        let mut ui = Ui::capture(Locale::En, OutputPolicy::plain(), &mut stdout, &mut stderr);
        global(&mut ui, status)
    };
    Ok(Printed {
        code,
        stdout: String::from_utf8(stdout).required_because("UTF-8")?,
        stderr: String::from_utf8(stderr).required_because("UTF-8")?,
    })
}

/// 問題を1件も持たない診断結果。
fn healthy() -> GlobalStatus {
    GlobalStatus {
        rows: vec![
            Row {
                item: "status-item-config",
                status: StatusValue::Ready,
            },
            Row {
                item: "status-item-daemon",
                status: StatusValue::Stopped,
            },
        ],
        diagnostics: Vec::new(),
        warnings: Vec::new(),
    }
}

fn ignored_key(key: &str) -> Warning {
    Warning::text(crate::msg!(
        "warning-config-unknown-key",
        path = "/home/example/.sbxm/config.yaml",
        key = key
    ))
}

fn unreachable_engine() -> Diagnostic {
    Diagnostic::new(
        ErrorId::DockerUnreachable,
        crate::msg!("error-docker-unreachable"),
    )
}

fn unusable_state() -> Diagnostic {
    Diagnostic::new(
        ErrorId::GlobalStateUnusable,
        crate::msg!("error-global-state-unusable"),
    )
}

#[test]
fn a_report_without_a_problem_succeeds_and_leaves_stderr_untouched() -> Checked {
    let printed = print(&healthy())?;

    assert_eq!(printed.code, ExitCode::Success);
    assert!(printed.stderr.is_empty(), "{:?}", printed.stderr);
    // 表そのものが結論であるため、健全なhostでも行は出る。
    assert!(printed.stdout.contains("GLOBAL"), "{:?}", printed.stdout);
    assert!(
        printed.stdout.contains(StatusValue::Stopped.as_str()),
        "{:?}",
        printed.stdout
    );
    Ok(())
}

#[test]
fn a_warning_is_shown_beside_the_result_without_deciding_the_exit_code() -> Checked {
    let mut status = healthy();
    status.warnings.push(ignored_key("colour"));

    let printed = print(&status)?;

    // 読み飛ばしたkeyは伝えるが、診断ではないため成功を取り消さない。
    assert_eq!(printed.code, ExitCode::Success);
    assert!(
        printed.stderr.contains("! Warning: "),
        "{:?}",
        printed.stderr
    );
    assert!(printed.stderr.contains("colour"), "{:?}", printed.stderr);
    // 結果表だけがstdoutにあり、注意がそこへ混ざらない。
    assert!(!printed.stdout.contains("Warning"), "{:?}", printed.stdout);
    Ok(())
}

#[test]
fn every_warning_reaches_the_reader_rather_than_only_the_first() -> Checked {
    let mut status = healthy();
    status.warnings.push(ignored_key("colour"));
    status.warnings.push(ignored_key("editor"));

    let printed = print(&status)?;

    assert_eq!(
        printed.stderr.matches("! Warning: ").count(),
        2,
        "{:?}",
        printed.stderr
    );
    Ok(())
}

#[test]
fn a_diagnosed_host_fails_and_every_diagnostic_is_written_on_its_own() -> Checked {
    let mut status = healthy();
    status.diagnostics.push(unreachable_engine());
    status.diagnostics.push(unusable_state());

    let printed = print(&status)?;

    assert_eq!(printed.code, ExitCode::Failure);
    assert_eq!(
        printed.stderr.matches("\u{d7} error:").count(),
        2,
        "{:?}",
        printed.stderr
    );
    // 診断が出ても結果表は隠れない。
    assert!(printed.stdout.contains("GLOBAL"), "{:?}", printed.stdout);
    assert!(!printed.stdout.contains("error:"), "{:?}", printed.stdout);
    Ok(())
}
