use crate::diagnostics::Msg;

use crate::design::text::CommandLine;

/// 失敗をどう解消するか。
///
/// 「同じcommandをもう一度実行する」のように実際のargvを組み立てられない案内は、
/// 架空のcommandを作らず説明だけを出す。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Remediation {
    pub explanation: Vec<Msg>,
    pub commands: Vec<CommandLine>,
}

impl Remediation {
    pub fn new() -> Remediation {
        Remediation::default()
    }

    /// 説明を1件だけ持つ対処方法。
    pub fn text(explanation: Msg) -> Remediation {
        Remediation::new().explain(explanation)
    }

    /// 説明を足す。
    pub fn explain(mut self, explanation: Msg) -> Remediation {
        self.explanation.push(explanation);
        self
    }

    /// 実行するcommandを足す。rendererが独立blockとして描画する。
    pub fn run(mut self, command: CommandLine) -> Remediation {
        self.commands.push(command);
        self
    }

    /// commandを組み立てられた場合だけ足す。
    pub fn try_run(self, command: impl Into<String>) -> Remediation {
        match CommandLine::optional(command) {
            Some(command) => self.run(command),
            None => self,
        }
    }
}

/// 説明だけの対処方法は、message1件からそのまま作れる。
///
/// commandを伴う対処だけがbuilderを必要とし、それ以外の呼び出し側は`Msg`を渡すだけで済む。
impl From<Msg> for Remediation {
    fn from(explanation: Msg) -> Remediation {
        Remediation::text(explanation)
    }
}
