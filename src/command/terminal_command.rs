use super::{CommandSpec, EnvPolicy, OutputPolicy, TimeoutClass};

/// 出力が端末まで届く外部command。
///
/// `CommandSpec`はcaptureしか表せない。端末へ届く指定をこの型でしか作れないようにして
/// おくと、`HostEnvironment::run`には端末へ出る余地が残らず、外部toolの出力は必ず
/// [`ExternalOutput`]を通る。境界の空行を置き忘れる場所が、型の上で無くなる。
///
/// [`ExternalOutput`]: crate::design::ExternalOutput
#[derive(Debug, Clone)]
pub struct TerminalCommand {
    spec: CommandSpec,
}

impl TerminalCommand {
    /// 外部toolの出力をsbxmが中継するcommand。
    ///
    /// 長時間かかる工程の進捗を実行中に見せるために使う。sbxmは進捗の実況を重ねない。
    pub fn relayed(program: &str, args: &[&str]) -> TerminalCommand {
        TerminalCommand {
            spec: CommandSpec {
                output: OutputPolicy::Relay,
                ..CommandSpec::capture(program, args)
            },
        }
    }

    /// 端末そのものを引き渡すcommand。
    ///
    /// SSH接続のように、利用者が入力する対話processへ使う。
    pub fn handed_over(program: &str, args: &[&str]) -> TerminalCommand {
        TerminalCommand {
            spec: CommandSpec {
                output: OutputPolicy::HandOver,
                timeout: TimeoutClass::Interactive,
                ..CommandSpec::capture(program, args)
            },
        }
    }

    pub fn env(mut self, policy: EnvPolicy) -> TerminalCommand {
        self.spec = self.spec.env(policy);
        self
    }

    pub fn timeout(mut self, class: TimeoutClass) -> TerminalCommand {
        self.spec = self.spec.timeout(class);
        self
    }

    pub fn spec(&self) -> &CommandSpec {
        &self.spec
    }
}
