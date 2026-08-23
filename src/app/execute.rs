use std::path::Path;

use crate::command::RealHost;
use crate::commands::{Command, Context};
use crate::design::{Document, PromptUi, Ui};
use crate::diagnostics::{ExitCode, Result};
use crate::support::sandbox::WORKSPACE_ROOT;

use super::invocation::Invocation;

/// parse結果を利用者へ提示し、通常commandをapplication境界から実行する。
///
/// この1回の起動が持つ材料はここで尽きる。`Invocation`は値で受け取り、実行の間だけ
/// 保持する。
#[allow(clippy::needless_pass_by_value)]
pub(super) fn execute(invocation: Invocation, command: Result<Command>) -> ExitCode {
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
        // helpとversionは、設定も対話も外部環境も要らない提示である。
        Command::Help(text) | Command::Version(text) => {
            ui.stdout(&Document::new().verbatim(text));
            ExitCode::Success
        }
        command => {
            let context = Context {
                location: invocation.location(),
                workspace_root: Path::new(WORKSPACE_ROOT),
                locale,
                can_prompt: invocation.can_prompt(),
            };
            let host = &RealHost;
            let mut prompt = PromptUi::terminal(locale, policy.stderr);
            match command {
                // 上の腕が取り切っているため、ここへは届かない。
                Command::Help(_) | Command::Version(_) => ExitCode::Success,
                Command::Add(args) => {
                    crate::commands::add::exec(&args, &context, &mut ui, host, &mut prompt)
                }
                Command::Apply(args) => {
                    crate::commands::apply::exec(&args, &context, &mut ui, host, &mut prompt)
                }
                Command::Prepare(project) => crate::commands::prepare::exec(
                    project.as_ref(),
                    &context,
                    &mut ui,
                    host,
                    &mut prompt,
                ),
                Command::Rebuild(project) => crate::commands::rebuild::exec(
                    project.as_ref(),
                    &context,
                    &mut ui,
                    host,
                    &mut prompt,
                ),
                Command::Open(args) => {
                    crate::commands::open::exec(&args, &context, &mut ui, host, &mut prompt)
                }
                Command::Stop(projects) => {
                    crate::commands::stop::exec(&projects, &context, &mut ui, host, &mut prompt)
                }
                Command::Ls => crate::commands::ls::exec(&context, &mut ui, host),
                Command::Status(scope) => {
                    crate::commands::status::exec(&scope, &context, &mut ui, host, &mut prompt)
                }
                Command::Destroy(args) => {
                    crate::commands::destroy::exec(&args, &context, &mut ui, host, &mut prompt)
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "execute_test.rs"]
mod execute_test;
