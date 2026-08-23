use crate::app::invocation::CommandLine;
use crate::diagnostics::{Error, ExitCode};
use crate::testing::cli::argv;

use super::report_startup_error;

/// 起動失敗は、argvを解釈する前の結果である。
///
/// 解釈すればhelpとして成功する引数も、別の診断で失敗する引数も、同じ起動失敗として
/// 終わる。full parseへ進んでいないことは、この差が出ないことで分かる。
#[test]
fn startup_errors_are_reported_without_an_invocation() {
    for arguments in [
        vec!["--color=never"],
        vec!["--color=never", "--help"],
        vec!["--color=never", "--version"],
        vec!["--color=never", "--lang=zz", "ls"],
        vec!["--color=never", "teleport"],
    ] {
        let command_line = CommandLine::new(argv(&arguments));

        assert_eq!(
            report_startup_error(&command_line, &Error::Canceled),
            ExitCode::Canceled,
            "{arguments:?}"
        );
    }
}
