//! `rebuild`の実行と出力。

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::hash::short_hex;
use crate::msg;
use crate::project::ProjectId;
use crate::support::display::format_or_report;
use crate::support::{Reporter, inventory, sandbox};

use super::super::{Context, report};

pub fn exec(project: &ProjectId, context: &Context) -> ExitCode {
    let (config, catalog) = match context.require_config() {
        Ok(pair) => pair,
        Err(error) => return report(&context.fallback_catalog(), &error),
    };
    let output = match super::run::run(
        &config,
        project,
        &RealHost,
        std::path::Path::new(sandbox::WORKSPACE_ROOT),
        inventory::Poll::default(),
    ) {
        Ok(output) => output,
        Err(error) => return report(&catalog, &error),
    };

    let reporter = Reporter::new(&catalog);
    let mut stderr = std::io::stderr();
    for warning in &output.warnings {
        reporter.print_warning(warning, &mut stderr);
    }
    let message = if output.unchanged {
        msg!("rebuild-unchanged", project = output.project)
    } else {
        msg!(
            "rebuild-applied",
            project = output.project,
            sandbox = output.sandbox,
            generation = short_hex(&output.applied)
        )
    };
    println!("{}", format_or_report(&catalog, &message));
    ExitCode::Success
}
