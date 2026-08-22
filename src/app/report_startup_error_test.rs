use crate::app::invocation::CommandLine;
use crate::diagnostics::{Error, ExitCode};
use crate::testing::cli::argv;

use super::report_startup_error;

#[test]
fn startup_errors_are_reported_without_an_invocation() {
    let command_line = CommandLine::new(argv(&["--color=never"]));
    let error = Error::Canceled;

    assert_eq!(
        report_startup_error(&command_line, &error),
        ExitCode::Canceled
    );
}
