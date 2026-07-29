//! `destroy`の実行。
//!
//! 消す前に計画を見せ、確認を取ってから実行する。

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::msg;
use crate::support::Reporter;
use crate::support::display::format_or_report;
use crate::support::select::TerminalProjectPrompt;
use crate::support::{inventory, sandbox};

use super::super::{Context, report};
use super::run::TerminalConfirmPrompt;
use super::{Args, print};

pub fn exec(args: &Args, context: &Context) -> ExitCode {
    let (config, catalog) = match context.require_config() {
        Ok(pair) => pair,
        Err(error) => return report(&context.fallback_catalog(), &error),
    };
    let mut prompt = TerminalProjectPrompt {
        heading: "select-destroy-heading",
        locale: catalog.locale(),
    };
    let prepared = match super::run::prepare(
        &config,
        args.project.as_ref(),
        args.force,
        &RealHost,
        &mut prompt,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
    ) {
        Ok(prepared) => prepared,
        Err(error) => return report(&catalog, &error),
    };

    print::plan(&catalog, &prepared.plan);
    let mut confirm = TerminalConfirmPrompt {
        locale: catalog.locale(),
    };
    if let Err(error) =
        super::run::confirm(&prepared, context.interactivity.can_prompt(), &mut confirm)
    {
        return report(&catalog, &error);
    }

    let outcome = match super::run::execute(&RealHost, &prepared, inventory::Poll::default()) {
        Ok(outcome) => outcome,
        Err(error) => return report(&catalog, &error),
    };

    let reporter = Reporter::new(&catalog);
    let mut stderr = std::io::stderr();
    for warning in &outcome.warnings {
        reporter.print_warning(warning, &mut stderr);
    }
    println!(
        "{}",
        format_or_report(&catalog, &msg!("destroy-done", project = outcome.project))
    );
    println!(
        "{}",
        format_or_report(
            &catalog,
            &msg!("destroy-re-register", command = outcome.re_register)
        )
    );
    ExitCode::Success
}
