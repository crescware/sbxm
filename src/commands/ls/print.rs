//! `ls`の出力。

use std::io::Write;

use crate::i18n::Catalog;
use crate::support::Reporter;
use crate::support::display::text_or_report;

use super::run::Listing;

/// `ls`の出力。
pub fn listing(catalog: &Catalog, listing: &Listing) {
    let reporter = Reporter::new(catalog);
    let projects: Vec<Vec<String>> = listing
        .projects
        .iter()
        .map(|row| {
            vec![
                row.project.clone(),
                row.sandbox.clone(),
                row.state.as_str().to_string(),
            ]
        })
        .collect();
    println!("{}", text_or_report(catalog, "ls-projects-section"));
    print!(
        "{}",
        reporter.render_value_table(
            &["column-project", "column-sandbox", "column-state"],
            &projects,
        )
    );

    if !listing.unmanaged.is_empty() {
        let unmanaged: Vec<Vec<String>> = listing
            .unmanaged
            .iter()
            .map(|row| {
                vec![
                    row.sandbox.clone(),
                    row.state.clone(),
                    row.workspace.clone(),
                ]
            })
            .collect();
        println!("\n{}", text_or_report(catalog, "ls-unmanaged-section"));
        print!(
            "{}",
            reporter.render_value_table(
                &["column-sandbox", "column-state", "column-workspace"],
                &unmanaged,
            )
        );
    }

    let values: Vec<(&str, &str)> = listing
        .projects
        .iter()
        .map(|row| (row.state.as_str(), row.state.legend_id()))
        .collect();
    if let Some(legend) = reporter.render_value_legend(&values) {
        print!("\n{legend}");
    }
    let _ = std::io::stdout().flush();
}
