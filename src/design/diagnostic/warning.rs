use crate::diagnostics::Msg;

use crate::design::text::CommandLine;

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
