use std::path::{Path, PathBuf};

use super::{CommandInput, EnvPolicy, InputBytes, OutputPolicy, TimeoutClass};

/// 1回の外部command実行の指定。
///
/// 公開constructorはどれも出力をcaptureする。端末まで届く指定は[`TerminalCommand`]だけが
/// 作れる。
///
/// [`TerminalCommand`]: super::TerminalCommand
#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: EnvPolicy,
    pub timeout: TimeoutClass,
    pub(super) output: OutputPolicy,
    /// 作業directory。指定しない場合は現在processのcurrent directoryを継承する。
    pub working_dir: Option<PathBuf>,
    /// stdinへ渡すもの。
    pub(super) input: CommandInput,
}

impl CommandSpec {
    /// structured outputを読むread-only probe。
    pub fn probe(program: &str, args: &[&str]) -> CommandSpec {
        CommandSpec::capture(program, args).timeout(TimeoutClass::Probe)
    }

    /// 出力をparseする、または秘匿するcommand。
    pub fn capture(program: &str, args: &[&str]) -> CommandSpec {
        CommandSpec {
            program: program.to_string(),
            args: args.iter().map(|arg| (*arg).to_string()).collect(),
            env: EnvPolicy::Inherit,
            timeout: TimeoutClass::Probe,
            output: OutputPolicy::Capture,
            working_dir: None,
            input: CommandInput::Empty,
        }
    }

    /// 子processの出力の扱い。
    pub fn output(&self) -> OutputPolicy {
        self.output
    }

    pub fn env(mut self, policy: EnvPolicy) -> CommandSpec {
        self.env = policy;
        self
    }

    pub fn timeout(mut self, class: TimeoutClass) -> CommandSpec {
        self.timeout = class;
        self
    }

    /// stdinへ`bytes`を渡す。書き終えたらstdinを閉じる。
    pub fn with_input(mut self, bytes: Vec<u8>) -> CommandSpec {
        self.input = CommandInput::Bytes(InputBytes::new(bytes));
        self
    }

    /// stdinへ渡すbyte列。
    pub fn input(&self) -> Option<&[u8]> {
        match &self.input {
            CommandInput::Bytes(bytes) => Some(bytes.as_slice()),
            CommandInput::Empty | CommandInput::File(_) => None,
        }
    }

    /// stdinへ`path`のfileをつなぐ。sbxmはその中身を読まず、子が読み切ればEOFになる。
    ///
    /// memoryへ読み込まずに渡す、大きな入力のために使う。
    pub fn with_input_file(mut self, path: &Path) -> CommandSpec {
        self.input = CommandInput::File(path.to_path_buf());
        self
    }

    /// stdinへつなぐfile。
    pub fn input_file(&self) -> Option<&Path> {
        match &self.input {
            CommandInput::File(path) => Some(path),
            CommandInput::Empty | CommandInput::Bytes(_) => None,
        }
    }

    pub fn working_dir(mut self, directory: &Path) -> CommandSpec {
        self.working_dir = Some(directory.to_path_buf());
        self
    }
}
