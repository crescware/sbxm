use crate::commands::apply::{ProjectOutcome, ProjectResult};
use crate::design::{Fact, RenderingPolicy};
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

fn print(applied: &AllReport, locale: Locale) -> Checked<Printed> {
    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    let code = {
        let mut ui = Ui::capture(locale, RenderingPolicy::plain(), &mut stdout, &mut stderr);
        all_report(&mut ui, applied)
    };
    Ok(Printed {
        code,
        stdout: String::from_utf8(stdout).required_because("UTF-8")?,
        stderr: String::from_utf8(stderr).required_because("UTF-8")?,
    })
}

fn outcome(project: &str, result: ProjectResult) -> ProjectOutcome {
    ProjectOutcome {
        project: project.to_owned(),
        sandbox: format!("sbxm-{}", project.to_lowercase().replace('/', "-")),
        result,
    }
}

fn row<'a>(output: &'a str, project: &str) -> Checked<&'a str> {
    output
        .lines()
        .find(|line| line.contains(project))
        .required_because("the project row is printed")
}

#[test]
fn every_project_reaches_stdout_with_its_result() -> Checked {
    let applied = AllReport {
        outcomes: vec![
            outcome("Example-Org/Applied", ProjectResult::Applied),
            outcome("Example-Org/Unchanged", ProjectResult::Unchanged),
            outcome("Example-Org/Unbuilt", ProjectResult::NotCreated),
        ],
        failures: Vec::new(),
    };

    let printed = print(&applied, Locale::En)?;
    assert_eq!(printed.code, ExitCode::Success);
    assert!(row(&printed.stdout, "Example-Org/Applied")?.contains("applied"));
    assert!(row(&printed.stdout, "Example-Org/Unchanged")?.contains("unchanged"));
    assert!(row(&printed.stdout, "Example-Org/Unbuilt")?.contains("not-created"));
    assert!(printed.stderr.is_empty(), "{}", printed.stderr);
    // 停止中の案件が無ければ、届ける手順も要らない。
    assert!(
        !printed.stdout.contains("sbxm apply <project-id> --files"),
        "{}",
        printed.stdout
    );
    Ok(())
}

#[test]
fn a_stopped_project_is_followed_by_how_to_deliver_the_files() -> Checked {
    let applied = AllReport {
        outcomes: vec![outcome("Example-Org/Stopped", ProjectResult::Stopped)],
        failures: Vec::new(),
    };

    let printed = print(&applied, Locale::En)?;
    // 置かなかったことは失敗ではないが、宣言はまだ届いていない。
    assert_eq!(printed.code, ExitCode::Success);
    assert!(row(&printed.stdout, "Example-Org/Stopped")?.contains("stopped"));
    assert!(
        printed.stdout.contains("sbxm apply <project-id> --files"),
        "{}",
        printed.stdout
    );
    Ok(())
}

#[test]
fn one_failed_project_decides_the_exit_code_and_its_diagnostic_names_it() -> Checked {
    let applied = AllReport {
        outcomes: vec![
            outcome("Example-Org/Failed", ProjectResult::Failed),
            outcome("Example-Org/Applied", ProjectResult::Applied),
        ],
        failures: vec![
            Diagnostic::new(
                ErrorId::DeclaredFileModified,
                crate::msg!(
                    "error-declared-file-modified",
                    destination = "/home/agent/.gitconfig"
                ),
            )
            .fact(Fact::project("Example-Org/Failed")),
        ],
    };

    let printed = print(&applied, Locale::En)?;
    assert_eq!(printed.code, ExitCode::Failure);
    assert!(row(&printed.stdout, "Example-Org/Applied")?.contains("applied"));
    assert!(row(&printed.stdout, "Example-Org/Failed")?.contains("failed"));
    assert!(
        printed.stderr.contains("declared-file-modified"),
        "{}",
        printed.stderr
    );
    assert!(
        printed.stderr.contains("Example-Org/Failed"),
        "{}",
        printed.stderr
    );
    Ok(())
}

#[test]
fn every_result_carries_its_own_explanation_outside_the_source_locale() -> Checked {
    let applied = AllReport {
        outcomes: vec![
            outcome("Example-Org/Applied", ProjectResult::Applied),
            outcome("Example-Org/Unchanged", ProjectResult::Unchanged),
            outcome("Example-Org/Stopped", ProjectResult::Stopped),
            outcome("Example-Org/Unbuilt", ProjectResult::NotCreated),
            outcome("Example-Org/Failed", ProjectResult::Failed),
        ],
        failures: Vec::new(),
    };

    let printed = print(&applied, Locale::Ja)?;
    let catalog = crate::i18n::Catalog::new(Locale::Ja);
    for result in [
        ProjectResult::Applied,
        ProjectResult::Unchanged,
        ProjectResult::Stopped,
        ProjectResult::NotCreated,
        ProjectResult::Failed,
    ] {
        let explanation = catalog
            .text(result.legend_id())
            .required_because(&format!("{} has a legend", result.as_str()))?;
        assert!(
            printed.stdout.contains(&explanation),
            "{} is explained: {}",
            result.as_str(),
            printed.stdout
        );
    }
    Ok(())
}
