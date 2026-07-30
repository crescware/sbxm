//! warningとremediationの構造。
//!
//! 「説明」と「実行するcommand」を一つの翻訳messageへ混ぜない。混ぜると、command行を
//! 独立させるという不変条件をrendererが守れなくなり、翻訳者がcommandの綴りを預かる
//! ことにもなる。説明は翻訳resource、commandはRust側のmodelが持つ。

use crate::error::Msg;

use super::text::CommandLine;

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

/// 結果を隠さずに伝える注意。
///
/// 単純なwarningは説明だけを持つ。後続の操作がある場合も、rendererが必ず独立した
/// command blockにする。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub description: Msg,
    pub guidance: Vec<Msg>,
    pub commands: Vec<CommandLine>,
}

impl Warning {
    /// 説明だけのwarning。
    pub fn text(description: Msg) -> Warning {
        Warning {
            description,
            guidance: Vec::new(),
            commands: Vec::new(),
        }
    }

    /// 補足を足す。
    pub fn explain(mut self, guidance: Msg) -> Warning {
        self.guidance.push(guidance);
        self
    }

    /// 後続の操作を足す。
    pub fn run(mut self, command: CommandLine) -> Warning {
        self.commands.push(command);
        self
    }

    /// commandを組み立てられた場合だけ足す。
    pub fn try_run(self, command: impl Into<String>) -> Warning {
        match CommandLine::optional(command) {
            Some(command) => self.run(command),
            None => self,
        }
    }
}
