use crate::command::RealHost;
use crate::commands::{Command, Context};
use crate::design::{Document, PromptUi, Ui};
use crate::diagnostics::{ExitCode, Result};

use super::invocation::Invocation;

/// parse結果を利用者へ提示し、通常commandをapplication境界から実行する。
pub(super) fn execute(invocation: &Invocation, command: Result<Command>) -> ExitCode {
    let locale = invocation.locale();
    let policy = invocation.rendering_policy();
    let mut ui = Ui::terminal(locale, policy);

    let command = match command {
        Ok(command) => command,
        Err(error) => {
            ui.error(&error);
            return error.exit_code();
        }
    };

    match command {
        Command::Help(text) | Command::Version(text) => {
            ui.stdout(&Document::new().verbatim(text));
            ExitCode::Success
        }
        command => {
            let context = Context {
                location: invocation.location(),
                workspace_root: std::path::Path::new(crate::support::sandbox::WORKSPACE_ROOT),
                locale,
                can_prompt: invocation.can_prompt(),
            };
            let mut prompt = PromptUi::terminal(locale, policy.stderr);
            dispatch(command, &context, &mut ui, &RealHost, &mut prompt)
        }
    }
}

fn dispatch(
    command: Command,
    context: &Context,
    ui: &mut Ui,
    host: &dyn crate::command::HostEnvironment,
    prompt: &mut PromptUi,
) -> ExitCode {
    match command {
        Command::Help(_) | Command::Version(_) => ExitCode::Success,
        Command::Add(args) => crate::commands::add::exec(&args, context, ui, host, prompt),
        Command::Apply(args) => crate::commands::apply::exec(&args, context, ui, host, prompt),
        Command::Prepare(project) => {
            crate::commands::prepare::exec(project.as_ref(), context, ui, host, prompt)
        }
        Command::Rebuild(project) => {
            crate::commands::rebuild::exec(project.as_ref(), context, ui, host, prompt)
        }
        Command::Open(args) => crate::commands::open::exec(&args, context, ui, host, prompt),
        Command::Stop(projects) => {
            crate::commands::stop::exec(&projects, context, ui, host, prompt)
        }
        Command::Ls => crate::commands::ls::exec(context, ui, host),
        Command::Status(scope) => crate::commands::status::exec(&scope, context, ui, host, prompt),
        Command::Destroy(args) => crate::commands::destroy::exec(&args, context, ui, host, prompt),
    }
}

#[cfg(test)]
#[path = "execute_test.rs"]
mod execute_test;
