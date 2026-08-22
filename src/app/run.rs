use crate::config;
use crate::diagnostics::ExitCode;

use super::execute::execute;
use super::invocation::{CommandLine, Invocation};
use super::report_startup_error::report_startup_error;

pub(crate) fn run(argv: Vec<String>) -> ExitCode {
    let command_line = CommandLine::new(argv);
    let config = match config::observe() {
        Ok(config) => config,
        Err(error) => return report_startup_error(&command_line, &error),
    };
    let invocation = Invocation::new(command_line, config);
    let command = invocation.parse();
    execute(&invocation, command)
}
