//! `stop`の出力。

use std::io::Write;

use crate::error::{Error, ExitCode};
use crate::i18n::Catalog;
use crate::support::Reporter;

use super::run::StopReport;

/// `stop`の出力。
///
/// 対象ごとの結果を表示し、1件でも失敗していればexit code `1`とする。
pub fn report(catalog: &Catalog, stopped: &StopReport) -> ExitCode {
    let reporter = Reporter::new(catalog);
    let rows: Vec<Vec<String>> = stopped
        .outcomes
        .iter()
        .map(|outcome| {
            vec![
                outcome.project.clone(),
                outcome.sandbox.clone(),
                outcome.result.as_str().to_string(),
            ]
        })
        .collect();
    print!(
        "{}",
        reporter.render_value_table(
            &["column-project", "column-sandbox", "column-result"],
            &rows,
        )
    );

    let values: Vec<(&str, &str)> = stopped
        .outcomes
        .iter()
        .map(|outcome| (outcome.result.as_str(), outcome.result.legend_id()))
        .collect();
    if let Some(legend) = reporter.render_value_legend(&values) {
        print!("\n{legend}");
    }
    let _ = std::io::stdout().flush();

    let mut stderr = std::io::stderr();
    for diagnostic in &stopped.failures {
        reporter.print_error(&Error::single(diagnostic.clone()), &mut stderr);
    }
    if stopped.failures.is_empty() {
        ExitCode::Success
    } else {
        ExitCode::Failure
    }
}
