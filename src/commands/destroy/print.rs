//! `destroy`の出力。

use std::io::Write;

use crate::i18n::Catalog;
use crate::msg;
use crate::support::Reporter;
use crate::support::display::{format_or_report, text_or_report};

use super::run::{DestroyPlan, Target};

/// `destroy`が何を消し、何を残すかを削除前に見せる。
pub fn plan(catalog: &Catalog, plan: &DestroyPlan) {
    let reporter = Reporter::new(catalog);
    let mut stderr = std::io::stderr();

    print!(
        "{}",
        reporter.render_fields(&[
            ("add-field-project", plan.project.clone()),
            ("add-field-sandbox", plan.sandbox.clone()),
            ("add-field-sandbox-state", plan.state.as_str().to_string()),
        ])
    );

    if plan.force {
        let _ = writeln!(
            stderr,
            "{}",
            text_or_report(catalog, "destroy-force-notice")
        );
    } else if !plan.worktrees.is_empty() {
        let rows: Vec<Vec<String>> = plan
            .worktrees
            .iter()
            .map(|worktree| {
                vec![
                    worktree.relative.clone(),
                    worktree.kind.as_str().to_string(),
                    worktree.mode.as_str().to_string(),
                    worktree.branch.clone().unwrap_or_else(|| "-".to_string()),
                    worktree.head.clone(),
                    worktree.remote.as_str().to_string(),
                ]
            })
            .collect();
        println!("\n{}", text_or_report(catalog, "status-worktrees-section"));
        print!(
            "{}",
            reporter.render_value_table(
                &[
                    "column-path",
                    "column-kind",
                    "column-mode",
                    "column-branch",
                    "column-head",
                    "column-remote"
                ],
                &rows,
            )
        );
        // 状態値は翻訳しないため、正本locale以外では説明を添える。
        let values: Vec<(&str, &str)> = plan
            .worktrees
            .iter()
            .flat_map(|worktree| worktree.legends())
            .collect();
        if let Some(legend) = reporter.render_value_legend(&values) {
            print!("\n{legend}");
        }
    }

    println!("\n{}", text_or_report(catalog, "destroy-removes"));
    for target in &plan.removes {
        println!("  {}", describe(catalog, target));
    }
    println!("{}", text_or_report(catalog, "destroy-keeps"));
    for target in &plan.keeps {
        println!("  {}", describe(catalog, target));
    }
    // 消す前に、消したあと元へ戻す方法まで見せる。
    println!(
        "\n{}",
        format_or_report(
            catalog,
            &msg!("destroy-re-register", command = plan.re_register.clone())
        )
    );
    let _ = std::io::stdout().flush();
}

/// 削除対象・保持対象の1行。pathはそのまま、それ以外は説明を訳す。
fn describe(catalog: &Catalog, target: &Target) -> String {
    match target {
        Target::Path(path) => path.clone(),
        Target::Described(message) => format_or_report(catalog, message),
    }
}
