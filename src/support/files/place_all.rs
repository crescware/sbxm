use crate::boundary::host::HostEnvironment;
use crate::config::FileDeclaration;
use crate::diagnostics::Result;

use super::{Conflict, PlacedFile, place};

/// 宣言されたfileをSandboxへ配置する。
pub fn place_all(
    host: &dyn HostEnvironment,
    sandbox: &str,
    declarations: &[FileDeclaration],
    conflict: Conflict,
) -> Result<Vec<PlacedFile>> {
    let mut placed = Vec::with_capacity(declarations.len());
    for (index, declaration) in declarations.iter().enumerate() {
        placed.push(place(host, sandbox, index, declaration, conflict)?);
    }
    Ok(placed)
}
