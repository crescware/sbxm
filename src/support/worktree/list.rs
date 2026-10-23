use crate::diagnostics::Result;
use crate::project::SandboxLayout;

use crate::support::sandbox::Inside;

use super::{Entry, list_command, parse_list};

/// Sandbox内のworktree一覧を読む。
///
/// 一覧を読めない場合は、worktreeが無いことと区別して外部commandの失敗とする。
pub fn list(inside: &dyn Inside, layout: &SandboxLayout) -> Result<Vec<Entry>> {
    let command = list_command(layout);
    let borrowed: Vec<&str> = command.iter().map(String::as_str).collect();
    let outcome = inside.exec(&borrowed)?;
    let outcome = outcome.require_success()?;
    parse_list(&outcome.stdout_text())
}
