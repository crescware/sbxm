use crate::boundary::host::{CommandOutcome, HostEnvironment};
use crate::diagnostics::Result;

use crate::support::sandbox;

/// originが広告する全refを、指定した一時namespaceへfetchする。
///
/// 明示的なrefspecで全namespaceを取得するため、通常のremote-tracking refやローカルtag
/// を変更しない。呼び出し側は観測後にnamespaceを削除しなければならない。
pub fn refresh_origin_all_refs(
    host: &dyn HostEnvironment,
    sandbox: &str,
    git_dir: &str,
    destination_namespace: &str,
) -> Result<CommandOutcome> {
    let refspec = format!("+refs/*:{destination_namespace}*");
    let args = [
        "git",
        "--git-dir",
        git_dir,
        "fetch",
        "--prune",
        "--no-tags",
        "origin",
        &refspec,
    ];
    sandbox::exec(host, sandbox, &args)
}
