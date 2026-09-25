use std::path::PathBuf;

use crate::config::{self, ConfigLocation, FileDeclaration, SandboxHomeRelativePath};
use crate::diagnostics::Result;

use super::invalid_destination;

/// 配置先で指定した宣言を、global configから外す。Sandboxへ置いたfileには触れない。
pub fn remove(location: &ConfigLocation, destination: &str) -> Result<(PathBuf, FileDeclaration)> {
    let destination = SandboxHomeRelativePath::new(destination)
        .map_err(|reason| invalid_destination(destination, reason))?;
    config::remove_file_declaration(location, &destination)
}
