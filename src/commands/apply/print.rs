//! `apply`の出力。

use std::io::Write;

use crate::i18n::Catalog;
use crate::msg;
use crate::paths;
use crate::support::Reporter;
use crate::support::display::{format_or_report, placement_legend, print_notes, text_or_report};

use super::run::ApplyOutput;

/// `apply`の成功出力。
pub fn output(catalog: &Catalog, output: &ApplyOutput) {
    let reporter = Reporter::new(catalog);
    if let Some(count) = output.worktrees {
        println!(
            "{}",
            format_or_report(
                catalog,
                &msg!(
                    "apply-worktrees-done",
                    count = count,
                    project = output.project,
                    sandbox = output.sandbox
                )
            )
        );
    }
    print_notes(catalog, &output.notes);
    if output.files.is_empty() && output.worktrees.is_some() {
        return;
    }
    println!(
        "{}",
        format_or_report(
            catalog,
            &msg!(
                "apply-files-done",
                count = output.files.len(),
                project = output.project,
                sandbox = output.sandbox
            )
        )
    );

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
        let values: Vec<(&str, &str)> = output
            .files
            .iter()
            .map(|file| placement_legend(file.placement))
            .collect();
        if let Some(legend) = reporter.render_value_legend(&values) {
            print!("\n{legend}");
        }
    }
    let _ = std::io::stdout().flush();
}
