use crate::diagnostics::Result;

use crate::support::sandbox::Inside;

use super::git_protocol_command;

/// `gh`の`GitHub` `protocol`設定が設定済みかを、変更せずに観測する。
///
/// HTTPS以外の値も、設定済みとして答える。Sandboxの中で利用者が選んだ値であり、
/// 欠けているものではない。
pub fn observe_git_protocol(inside: &dyn Inside) -> Result<bool> {
    let command = git_protocol_command();
    let borrowed: Vec<&str> = command.iter().map(String::as_str).collect();
    let outcome = inside.exec(&borrowed)?;
    if !outcome.success() {
        return Ok(false);
    }
    Ok(!outcome.stdout_text().trim().is_empty())
}
