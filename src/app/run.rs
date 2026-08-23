use crate::config::{self, ConfigObservation};
use crate::diagnostics::{ExitCode, Result};

use super::execute::execute;
use super::invocation::{CommandLine, Invocation};
use super::report_startup_error::report_startup_error;

/// 起動の順序だけを持つ。
///
/// localeの優先順位、parser、Commandのvariant、Host、Ui/Promptの配線はここへ置かず、
/// `Invocation`と`execute`が持つ。
pub(crate) fn run(argv: Vec<String>) -> ExitCode {
    run_with_config(CommandLine::new(argv), config::observe())
}

/// 起動材料が揃った状態からの順序。
///
/// `config::observe`はprocessのhome directoryを読む。その発見失敗はprocessの環境変数を
/// 書き換えないと再現できず、`unsafe`を禁じたこのcrateのtestからは踏めない。観測結果を
/// 受け取る位置をここへ置き、発見に失敗した起動がfull parseへ進まないことをtestで固定する。
fn run_with_config(command_line: CommandLine, config: Result<ConfigObservation>) -> ExitCode {
    let config = match config {
        Ok(config) => config,
        Err(error) => return report_startup_error(&command_line, &error),
    };
    let invocation = Invocation::new(command_line, config);
    let command = invocation.parse();
    execute(invocation, command)
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;
