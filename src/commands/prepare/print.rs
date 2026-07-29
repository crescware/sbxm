//! `prepare`の出力。

use std::io::Write;

use crate::i18n::Catalog;
use crate::msg;
use crate::paths;
use crate::support::Reporter;
use crate::support::display::{
    format_or_report, mode_legend, placement_legend, print_notes, sandbox_state_legend,
    text_or_report,
};

use super::run::PrepareOutput;

/// `prepare`の成功出力。
pub fn output(catalog: &Catalog, output: &PrepareOutput) {
    let reporter = Reporter::new(catalog);
    let mut stderr = std::io::stderr();
    for warning in &output.warnings {
        reporter.print_warning(warning, &mut stderr);
    }

    if output.already_built {
        println!(
            "{}",
            format_or_report(
                catalog,
                &msg!("prepare-already-built", project = output.project)
            )
        );
    }

    print!(
        "{}",
        reporter.render_fields(&[
            ("add-field-project", output.project.clone()),
            ("add-field-sandbox", output.sandbox.clone()),
            ("add-field-creation-mode", output.mode.to_string()),
            ("add-field-start-branch", output.start_ref.clone()),
            (
                "add-field-managed-worktrees",
                output.worktrees.len().to_string()
            ),
            (
                "add-field-sandbox-state",
                output.sandbox_state.as_str().to_string()
            ),
        ])
    );

    let worktrees: Vec<Vec<String>> = output
        .worktrees
        .iter()
        .map(|worktree| {
            vec![
                worktree.path.clone(),
                worktree.created_from.clone(),
                worktree.head.clone().unwrap_or_else(|| "-".to_string()),
                worktree.mode.to_string(),
            ]
        })
        .collect();
    if !worktrees.is_empty() {
        print!(
            "\n{}",
            reporter.render_value_table(
                &[
                    "column-worktree",
                    "column-created-from",
                    "column-head",
                    "column-mode"
                ],
                &worktrees,
            )
        );
    }

    let files: Vec<Vec<String>> = output
        .files
        .iter()
        .map(|file| {
            vec![
                paths::display(&file.source),
                file.destination.clone(),
                file.placement.as_str().to_string(),
            ]
        })
        .collect();
    if !files.is_empty() {
        print!(
            "\n{}",
            reporter.render_value_table(
                &["column-file", "column-destination", "column-result"],
                &files,
            )
        );
        println!("{}", text_or_report(catalog, "files-secret-hint"));
    }

    print_notes(catalog, &output.notes);

    let mut values: Vec<(&str, &str)> = vec![(
        output.sandbox_state.as_str(),
        sandbox_state_legend(output.sandbox_state),
    )];
    for worktree in &output.worktrees {
        values.push(mode_legend(worktree.mode));
    }
    for file in &output.files {
        values.push(placement_legend(file.placement));
    }
    if let Some(legend) = reporter.render_value_legend(&values) {
        print!("\n{legend}");
    }
    let _ = std::io::stdout().flush();
}
