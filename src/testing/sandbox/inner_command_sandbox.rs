use crate::command::{CommandOutcome, CommandSpec, HostEnvironment};
use crate::diagnostics::{Error, ErrorId, Result};
use crate::msg;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

/// `--`より後ろのinner commandをkeyにして答えるhost。
///
/// `crate::testing::host::FakeSbx`が引数全体をkeyにするのに対し、こちらは
/// `sbx exec <name> --`より後ろだけを見る。同じinner commandであれば、どのSandboxへ
/// 送られたものでも同じ応答を返す。
#[derive(Default)]
pub struct InnerCommandSandbox {
    /// Sandbox内に存在するpath。
    present: RefCell<Vec<String>>,
    /// 特定のinner commandに対する応答。
    answers: HashMap<String, (i32, String)>,
    /// 応答そのものが返らないinner command。
    timeouts: HashSet<String>,
    calls: RefCell<Vec<Vec<String>>>,
}

impl InnerCommandSandbox {
    pub fn new() -> InnerCommandSandbox {
        InnerCommandSandbox::default()
    }

    pub fn holding(mut self, paths: &[&str]) -> InnerCommandSandbox {
        self.present = RefCell::new(paths.iter().map(|path| (*path).to_string()).collect());
        self
    }

    pub fn answering(mut self, command: &str, stdout: &str) -> InnerCommandSandbox {
        self.answers
            .insert(command.to_string(), (0, stdout.to_string()));
        self
    }

    pub fn failing(mut self, command: &str) -> InnerCommandSandbox {
        self.answers.insert(command.to_string(), (1, String::new()));
        self
    }

    /// commandが終了statusを返さないSandbox。
    ///
    /// `failing`が「実行できて失敗した」を作るのに対し、こちらは実行そのものが
    /// 成立しなかった場合を作る。observationが1件も得られていない状態である。
    pub fn timing_out(mut self, command: &str) -> InnerCommandSandbox {
        self.timeouts.insert(command.to_string());
        self
    }

    pub fn calls(&self) -> Vec<Vec<String>> {
        self.calls.borrow().clone()
    }

    pub fn ran(&self, needle: &str) -> bool {
        self.calls()
            .iter()
            .any(|args| args.join(" ").contains(needle))
    }
}

impl HostEnvironment for InnerCommandSandbox {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.calls.borrow_mut().push(spec.args.clone());
        let inner = crate::testing::command::inner_args(spec);
        let key = inner.join(" ");

        if self.timeouts.contains(&key) {
            return Err(Error::new(
                ErrorId::ExternalCommandTimeout,
                msg!(
                    "error-external-command-timeout",
                    program = spec.program,
                    seconds = 10
                ),
            ));
        }

        let (code, stdout) = if inner.first().is_some_and(|arg| *arg == "test") {
            let target = inner.last().copied().unwrap_or_default();
            (
                i32::from(!self.present.borrow().iter().any(|known| known == target)),
                String::new(),
            )
        } else if key.contains("worktree add") {
            // 作成に成功したworktreeは、以後存在するものとして扱う。
            let path = inner
                .iter()
                .find(|arg| arg.contains(".tree-"))
                .copied()
                .unwrap_or_default();
            self.present.borrow_mut().push(path.to_string());
            (0, String::new())
        } else {
            match self.answers.get(&key) {
                Some((code, stdout)) => (*code, stdout.clone()),
                None => (0, String::new()),
            }
        };

        Ok(crate::testing::command::outcome(spec, code, &stdout))
    }
}
