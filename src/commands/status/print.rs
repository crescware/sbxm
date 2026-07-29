//! `status`の出力。

use std::io::Write;

use crate::error::{Error, ExitCode};
use crate::i18n::Catalog;
use crate::support::Reporter;
use crate::support::display::text_or_report;

use super::global::GlobalStatus;
use super::project::ProjectStatus;

/// global scopeの`status`の出力。
pub fn global(catalog: &Catalog, status: &GlobalStatus) -> ExitCode {
    let reporter = Reporter::new(catalog);

    let table = reporter.render_status_table(
        "status-global-section",
        "status-column-item",
        "status-column-status",
        &status.rows,
    );
    print!("{table}");
    if let Some(legend) = reporter.render_legend(&status.rows) {
        print!("\n{legend}");
    }
    let _ = std::io::stdout().flush();

    let mut stderr = std::io::stderr();
    for warning in &status.warnings {
        reporter.print_warning(warning, &mut stderr);
    }
    for diagnostic in &status.diagnostics {
        reporter.print_error(&Error::single(diagnostic.clone()), &mut stderr);
    }

    if status.is_healthy() {
        ExitCode::Success
    } else {
        ExitCode::Failure
    }
}

/// project scopeの`status`の出力。
///
/// 指定案件だけを診断し、global環境の検査結果を混ぜない。
pub fn project(catalog: &Catalog, status: &ProjectStatus) -> ExitCode {
    let reporter = Reporter::new(catalog);

    let mut fields: Vec<(&str, String)> = vec![("status-item-project", status.project.clone())];
    fields.extend(
        status
            .items
            .iter()
            .map(|item| (item.item, item.value.as_str().to_string())),
    );
    println!("{}", text_or_report(catalog, "status-project-section"));
    print!(
        "{}",
        reporter.render_value_table(
            &["status-column-item", "status-column-value"],
            &fields
                .iter()
                .map(|(item, value)| vec![text_or_report(catalog, item), value.clone()])
                .collect::<Vec<_>>(),
        )
    );

    let rows: Vec<Vec<String>> = status
        .worktrees
        .iter()
        .map(|worktree| {
            vec![
                worktree.path.clone(),
                worktree.kind.to_string(),
                worktree.mode.as_str().to_string(),
                worktree.state.as_str().to_string(),
            ]
        })
        .collect();
    println!("\n{}", text_or_report(catalog, "status-worktrees-section"));
    print!(
        "{}",
        reporter.render_value_table(
            &["column-path", "column-kind", "column-mode", "column-state"],
            &rows,
        )
    );

    let mut values: Vec<(&str, &str)> = status
        .items
        .iter()
        .map(|item| (item.value.as_str(), item.value.legend_id()))
        .collect();
    for worktree in &status.worktrees {
        values.push((worktree.mode.as_str(), worktree.mode.legend_id()));
        values.push((worktree.state.as_str(), worktree.state.legend_id()));
    }
    if let Some(legend) = reporter.render_value_legend(&values) {
        print!("\n{legend}");
    }
    let _ = std::io::stdout().flush();

    let mut stderr = std::io::stderr();
    for diagnostic in &status.diagnostics {
        reporter.print_error(&Error::single(diagnostic.clone()), &mut stderr);
    }
    if status.is_healthy() {
        ExitCode::Success
    } else {
        ExitCode::Failure
    }
}
