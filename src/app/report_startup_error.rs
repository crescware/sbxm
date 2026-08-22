use crate::design::{RenderingPolicy, Ui};
use crate::diagnostics::{Error, ExitCode};
use crate::i18n::Locale;

use super::invocation::CommandLine;

/// config locationだけが見つからない起動失敗を、Invocationなしで報告する。
pub(crate) fn report_startup_error(command_line: &CommandLine, error: &Error) -> ExitCode {
    let policy = RenderingPolicy::detect(command_line.color_mode());
    let mut ui = Ui::terminal(Locale::SOURCE, policy);
    ui.error(error);
    error.exit_code()
}

#[cfg(test)]
#[path = "report_startup_error_test.rs"]
mod report_startup_error_test;
