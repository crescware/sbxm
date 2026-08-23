use std::path::PathBuf;

use crate::app::invocation::CommandLine;
use crate::config::{ConfigLocation, ConfigObservation};
use crate::diagnostics::{Error, ExitCode};
use crate::testing::cli::argv;

use super::run_with_config;

fn observation() -> ConfigObservation {
    ConfigObservation::new(
        ConfigLocation::from_home(PathBuf::from("/nonexistent/test-home")),
        None,
    )
}

/// config locationを発見できない起動は、argvを解釈する前に終わる。
///
/// 同じargvが、観測に成功すればversionとして解釈され成功する。発見に失敗したときにその
/// 差が出ないことが、full parseへ進んでいないことを示す。
#[test]
fn configuration_discovery_failure_is_reported_before_the_arguments_are_parsed() {
    for arguments in [
        vec!["--color=never", "--version"],
        vec!["--color=never", "--lang=zz", "ls"],
        vec!["--color=never", "teleport"],
    ] {
        assert_eq!(
            run_with_config(CommandLine::new(argv(&arguments)), Err(Error::Canceled)),
            ExitCode::Canceled,
            "{arguments:?}"
        );
    }

    assert_eq!(
        run_with_config(
            CommandLine::new(argv(&["--color=never", "--version"])),
            Ok(observation())
        ),
        ExitCode::Success
    );
}
