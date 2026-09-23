use std::path::PathBuf;

use crate::config::{self, ConfigLocation, FileDeclaration, SandboxHomeRelativePath};
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::msg;

/// 配置先で指定した宣言を、global configから外す。Sandboxへ置いたfileには触れない。
pub fn remove(location: &ConfigLocation, destination: &str) -> Result<(PathBuf, FileDeclaration)> {
    let destination = SandboxHomeRelativePath::new(destination).map_err(|reason| {
        Error::single(
            Diagnostic::new(
                ErrorId::FileDeclarationInvalidDestination,
                msg!("error-file-declaration-invalid-destination"),
            )
            .fact(Fact::destination(destination))
            .fact(Fact::reason(reason)),
        )
    })?;
    config::remove_file_declaration(location, &destination)
}
