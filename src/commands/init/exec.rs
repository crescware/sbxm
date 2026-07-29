//! `init`の実行と出力。

use crate::command::RealHost;
use crate::error::ExitCode;
use crate::i18n::Catalog;
use crate::msg;
use crate::paths;
use crate::support::Reporter;
use crate::support::display::{format_or_report, text_or_report};

use super::super::{Context, report};
use super::Mode;
use super::run::{InitRequest, TerminalPrompt};

pub fn exec(mode: &Mode, context: &Context) -> ExitCode {
    let request = InitRequest {
        mode: mode.clone(),
        lang: context.lang,
        interactivity: context.interactivity,
    };
    let mut prompt = TerminalPrompt;
    let output = match super::run::run(context.location, &request, &RealHost, &mut prompt) {
        Ok(output) => output,
        Err(error) => return report(&context.fallback_catalog(), &error),
    };

    // 作成したconfigが選んだlocaleで、結果と次の一歩を出す。
    let catalog = Catalog::new(output.locale);
    let reporter = Reporter::new(&catalog);
    let mut stderr = std::io::stderr();
    for warning in &output.warnings {
        reporter.print_warning(warning, &mut stderr);
    }

    let path = paths::display(&output.config_path);
    let message = if output.already_initialized {
        msg!("init-already-initialized", path = path)
    } else {
        msg!("init-created", path = path)
    };
    println!("{}", format_or_report(&catalog, &message));
    println!("{}", text_or_report(&catalog, "init-next-step"));
    ExitCode::Success
}
