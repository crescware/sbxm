use crate::boundary::host::HostEnvironment;
use crate::config::FileDeclaration;
use crate::diagnostics::Result;

use super::{Conflict, PlacedFile, plan_all};

/// 宣言されたfileをSandboxへ配置する。
///
/// すべての扱いを決めてから置く。置けないfileが1件でもあれば、1件も置かない。
pub fn place_all(
    host: &dyn HostEnvironment,
    sandbox: &str,
    declarations: &[FileDeclaration],
    conflict: Conflict,
) -> Result<Vec<PlacedFile>> {
    plan_all(host, sandbox, declarations, conflict)?
        .iter()
        .map(|planned| planned.carry_out(host, sandbox))
        .collect()
}
