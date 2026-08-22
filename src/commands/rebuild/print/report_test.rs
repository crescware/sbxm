use crate::design::{RenderingPolicy, Ui, Warning};
use crate::i18n::Locale;

use crate::testing::outcome::{Checked, Required};

use super::*;

/// 世代のhash全体。表示は先頭だけを使う。
const APPLIED: &str = "4a0f8d41e27e53198137451dd09bc8aa8b8704b1f879a77655d643302029e33a";

/// 2つのstreamを別々に受け取る。結果と注意の行き先が入れ替わっても気付けるようにする。
struct Printed {
    code: ExitCode,
    stdout: String,
    stderr: String,
}

fn output(warnings: Vec<Warning>) -> RebuildOutput {
    RebuildOutput {
        project: "Example-Org/Example-Repo".to_string(),
        sandbox: "sbxm-example-org-example-repo-99a40327a69b".to_string(),
        applied: APPLIED.to_string(),
        warnings,
    }
}

fn moved_on() -> Warning {
    Warning::text(crate::msg!(
        "warning-dockerfile-changed-during-rebuild",
        project = "Example-Org/Example-Repo"
    ))
    .try_run("sbxm rebuild Example-Org/Example-Repo")
}

fn print(output: &RebuildOutput) -> Checked<Printed> {
    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    let code = {
        let mut ui = Ui::capture(
            Locale::En,
            RenderingPolicy::plain(),
            &mut stdout,
            &mut stderr,
        );
        report(&mut ui, output)
    };
    Ok(Printed {
        code,
        stdout: String::from_utf8(stdout).required_because("the result is UTF-8")?,
        stderr: String::from_utf8(stderr).required_because("the warnings are UTF-8")?,
    })
}

#[test]
fn a_rebuild_without_warnings_writes_the_result_and_leaves_stderr_untouched() -> Checked {
    let printed = print(&output(Vec::new()))?;

    assert_eq!(printed.code, ExitCode::Success);
    assert!(
        printed.stdout.contains("4a0f8d41e27e"),
        "{}",
        printed.stdout
    );
    assert!(printed.stderr.is_empty(), "{:?}", printed.stderr);
    Ok(())
}

#[test]
fn a_warning_is_written_beside_the_result_rather_than_in_its_place() -> Checked {
    let printed = print(&output(vec![moved_on()]))?;

    // 注意が付いても、適用した世代の報告は消えない。
    assert_eq!(printed.code, ExitCode::Success);
    assert!(
        printed.stdout.contains("4a0f8d41e27e"),
        "{}",
        printed.stdout
    );
    assert!(
        !printed.stdout.contains("Warning:"),
        "the result stream carries no notice: {}",
        printed.stdout
    );
    assert!(printed.stderr.contains("Warning:"), "{:?}", printed.stderr);
    assert!(
        printed
            .stderr
            .contains("sbxm rebuild Example-Org/Example-Repo"),
        "the notice keeps the command that resolves it: {:?}",
        printed.stderr
    );
    Ok(())
}

#[test]
fn every_warning_the_run_collected_is_written_as_its_own_block() -> Checked {
    let printed = print(&output(vec![moved_on(), moved_on()]))?;

    assert_eq!(
        printed.stderr.matches("Warning:").count(),
        2,
        "{:?}",
        printed.stderr
    );
    Ok(())
}
