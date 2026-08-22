use crate::config;
use crate::diagnostics::{ExitCode, Result};

use super::execute::execute;
use super::invocation::{CommandLine, Invocation};
use super::report_startup_error::report_startup_error;

pub(crate) fn run(argv: Vec<String>) -> ExitCode {
    let command_line = CommandLine::new(argv);
    run_with_config(command_line, config::observe())
}

fn run_with_config(
    command_line: CommandLine,
    config: Result<config::ConfigObservation>,
) -> ExitCode {
    let config = match config {
        Ok(config) => config,
        Err(error) => return report_startup_error(&command_line, &error),
    };
    let invocation = Invocation::new(command_line, config);
    let command = invocation.parse();
    execute(&invocation, command)
}

#[cfg(test)]
#[path = "run_test.rs"]
mod run_test;
