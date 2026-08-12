use crate::command::{EnvPolicy, PtyConfirmedCommand};
use crate::project::SandboxName;

/// 確認promptにだけ答えて`sbx rm`を実行する起動。
///
/// 確認prompt付きで`sbx`を動かす起動はここだけが作る。expected promptはsandbox名を
/// 含めて`sbx`が示す文言と完全に一致させ、それ以外のpromptには何も送らない。
pub fn remove_confirmed(name: &SandboxName) -> PtyConfirmedCommand {
    let expected = format!("Remove sandbox '{name}'? This cannot be undone. (y/N):");
    PtyConfirmedCommand::new("sbx", &["rm", name.as_str()], name.as_str(), &expected)
        .env(EnvPolicy::InheritWithoutSshAgent)
}
