use crate::diagnostics::Result;

use crate::support::sandbox::Inside;

/// `Sandbox内のGit` identityが設定済みかを、変更せずに観測する。
///
/// 記録と違う値も、設定済みとして答える。Sandboxの中で利用者が選んだ値であり、
/// 欠けているものではない。
///
/// 読むcommandは`observe_commands`と同じである。
pub fn observe(inside: &dyn Inside) -> Result<bool> {
    let name = observe_setting(inside, "user.name")?;
    let email = observe_setting(inside, "user.email")?;
    Ok(name && email)
}

fn observe_setting(inside: &dyn Inside, key: &str) -> Result<bool> {
    let outcome = inside.exec(&["git", "config", "--global", "--get", key])?;
    if !outcome.success() {
        return Ok(false);
    }
    Ok(!outcome.stdout_text().trim().is_empty())
}
