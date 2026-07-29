//! `add`の出力。

use std::io::Write;

use crate::i18n::Catalog;
use crate::msg;
use crate::paths;
use crate::support::Reporter;
use crate::support::display::{format_or_report, text_or_report};
use crate::support::secret;

use super::run::AddOutput;

/// `add`の成功出力。
///
/// 登録とhost cloneまでを示し、次にやることを続けて出す。GitHub tokenの登録先は
/// Sandbox名であり、その名前はここで確定する。
pub fn output(catalog: &Catalog, output: &AddOutput) {
    let reporter = Reporter::new(catalog);
    let mut stderr = std::io::stderr();
    for warning in &output.warnings {
        reporter.print_warning(warning, &mut stderr);
    }

    let message = if output.already_registered {
        msg!("add-already-registered", project = output.project)
    } else {
        msg!("add-registered", project = output.project)
    };
    println!("{}", format_or_report(catalog, &message));

    print!(
        "{}",
        reporter.render_fields(&[
            ("add-field-project", output.project.clone()),
            ("add-field-sandbox", output.sandbox.clone()),
            ("add-field-creation-mode", output.mode.to_string()),
            (
                "add-field-start-branch",
                output.start_ref.clone().unwrap_or_else(|| "-".to_string())
            ),
            (
                "add-field-managed-worktrees",
                output.requested_worktrees.to_string()
            ),
            ("add-field-host-clone", paths::display(&output.host_clone)),
        ])
    );

    println!("\n{}", text_or_report(catalog, "add-next-heading"));
    // tokenの権限は、失敗したときだけでなくここでも示す。暗記させない。
    println!("  {}", text_or_report(catalog, "add-next-token"));
    println!(
        "  {}",
        format_or_report(
            catalog,
            &msg!(
                "add-next-secret",
                command = secret::register_command(&output.sandbox, None)
            )
        )
    );
    println!(
        "  {}",
        format_or_report(
            catalog,
            &msg!(
                "add-next-prepare",
                command = format!("sbxm prepare {}", output.project)
            )
        )
    );
    let _ = std::io::stdout().flush();
}
