use crate::boundary::host::HostEnvironment;
use crate::boundary::host::protocol::CustomSecret;
use crate::diagnostics::Result;

use super::{covers_github_hosts, is_global_scope, list_customs};

/// このSandboxが使える、GitHub token の登録。
///
/// Sandboxへ結び付いた登録を優先し、無ければglobal scopeの登録を使う。ほかのSandbox
/// へ結び付いた登録は、このSandboxのrequestでは差し替えられないため候補にしない。
///
/// 同じ範囲に候補が複数あると、どのplaceholderをgitへ持たせるべきか決められない。
/// 片方を選ぶとtokenが食い違ったまま静かに失敗するため、選ばずに複数のまま返す。
pub(super) fn registered_github(
    host: &dyn HostEnvironment,
    sandbox: &str,
) -> Result<Vec<CustomSecret>> {
    let customs = list_customs(host)?;
    let scoped: Vec<CustomSecret> = customs
        .iter()
        .filter(|custom| custom.scope == sandbox && covers_github_hosts(custom))
        .cloned()
        .collect();
    if !scoped.is_empty() {
        return Ok(scoped);
    }
    Ok(customs
        .into_iter()
        .filter(|custom| is_global_scope(&custom.scope) && covers_github_hosts(custom))
        .collect())
}
