use crate::boundary::host::HostEnvironment;
use crate::config::FileDeclaration;
use crate::diagnostics::{Error, Result};

use super::{Conflict, PlannedFile, plan};

/// 宣言されたすべてのfileの扱いを、Sandboxを変えずに決める。
///
/// 上書きしないと判断したfileが1件でもあれば、その全件を並べて拒否する。利用者は1回の
/// 実行で、退避すべきものをすべて知れる。
pub fn plan_all(
    host: &dyn HostEnvironment,
    sandbox: &str,
    declarations: &[FileDeclaration],
    conflict: Conflict,
) -> Result<Vec<PlannedFile>> {
    let mut planned = Vec::with_capacity(declarations.len());
    let mut refused = Vec::new();
    for declaration in declarations {
        match plan(host, sandbox, declaration, conflict)? {
            Ok(file) => planned.push(file),
            Err(diagnostic) => refused.push(diagnostic),
        }
    }
    if refused.is_empty() {
        Ok(planned)
    } else {
        Err(Error::Diagnostics(refused))
    }
}
