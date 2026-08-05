use std::process::{Child, Command};

use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;

use super::{CommandSpec, spawn_failure};

/// 子processを起動する。相手が居ないことだけは、起動の失敗と別に述べる。
pub(super) fn spawn(command: &mut Command, spec: &CommandSpec) -> Result<Child> {
    command.spawn().map_err(|error| {
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
