use crate::app::invocation::CommandLine;
use crate::diagnostics::{Error, ExitCode};
use crate::testing::cli::argv;

use super::run_with_config;

#[test]
fn configuration_discovery_failure_is_reported_before_invocation_creation() {
    let command_line = CommandLine::new(argv(&["--color=never"]));

    assert_eq!(
        run_with_config(command_line, Err(Error::Canceled)),
        ExitCode::Canceled
    );
}
