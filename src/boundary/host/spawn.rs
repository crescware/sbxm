use std::process::{Child, Command};

use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;
use crate::paths;

use super::{CommandSpec, spawn_failure};

/// 子processを起動する。相手が居ないことと、作業directoryが無いことは、起動の失敗と
/// 別に述べる。
///
/// 作業directoryが無いときも同じ`NotFound`が返る。そのときは相手が居ないとは言わず、
/// 無いdirectoryを名指しする。
pub(super) fn spawn(command: &mut Command, spec: &CommandSpec) -> Result<Child> {
    command.spawn().map_err(|error| {
        if let Some(directory) = spec
            .working_dir
            .as_ref()
            .filter(|directory| !directory.is_dir())
        {
            return Error::single(
                Diagnostic::new(
                    ErrorId::ExternalCommandDirectoryMissing,
                    msg!(
                        "error-external-command-directory-missing",
                        program = spec.program
                    ),
                )
                .fact(Fact::directory(&paths::display(directory))),
            );
        }
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::new(
                ErrorId::ExternalCommandNotFound,
                msg!("error-external-command-not-found", program = spec.program),
            )
        } else {
            spawn_failure(spec, &error)
        }
    })
}
