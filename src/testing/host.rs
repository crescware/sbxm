//! Sandboxを持つhostのfake。

use crate::command::{CommandOutcome, CommandSpec, HostEnvironment};
use crate::error::Result;
use std::cell::RefCell;

/// Sandbox一覧を返し、実行された指定を記録するhost。
pub struct FakeSbx {
    pub listing: RefCell<Vec<String>>,
    pub answers: std::collections::HashMap<String, (i32, String)>,
    pub specs: RefCell<Vec<CommandSpec>>,
}

impl FakeSbx {
    pub fn listing(output: &str) -> FakeSbx {
        FakeSbx {
            listing: RefCell::new(vec![output.to_string()]),
            answers: std::collections::HashMap::new(),
            specs: RefCell::new(Vec::new()),
        }
    }

    /// 呼び出しごとに異なる一覧を返す。最後の1件は繰り返し使う。
    pub fn listings(outputs: &[&str]) -> FakeSbx {
        FakeSbx {
            listing: RefCell::new(
                outputs
                    .iter()
                    .rev()
                    .map(|value| value.to_string())
                    .collect(),
            ),
            answers: std::collections::HashMap::new(),
            specs: RefCell::new(Vec::new()),
        }
    }

    pub fn answering(mut self, command: &str, code: i32, stdout: &str) -> FakeSbx {
        self.answers
            .insert(command.to_string(), (code, stdout.to_string()));
        self
    }

    pub fn calls(&self) -> Vec<Vec<String>> {
        self.specs
            .borrow()
            .iter()
            .map(|spec| spec.args.clone())
            .collect()
    }

    pub fn ran(&self, needle: &str) -> bool {
        self.calls()
            .iter()
            .any(|args| args.join(" ").contains(needle))
    }

    /// 引数が一致した最後の1件の指定。envとoutput policyの検証に使う。
    pub fn spec(&self, needle: &str) -> CommandSpec {
        self.specs
            .borrow()
            .iter()
            .rev()
            .find(|spec| spec.args.join(" ").contains(needle))
            .unwrap_or_else(|| panic!("no command matched {needle}"))
            .clone()
    }
}

impl HostEnvironment for FakeSbx {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.specs.borrow_mut().push(spec.clone());
        let key = spec.args.join(" ");
        let (code, stdout) = if spec.args.first().is_some_and(|arg| arg == "ls") {
            let mut listings = self.listing.borrow_mut();
            let output = if listings.len() > 1 {
                listings.pop().unwrap_or_default()
            } else {
                listings.last().cloned().unwrap_or_default()
            };
            (0, output)
        } else {
            match self.answers.get(&key) {
                Some((code, stdout)) => (*code, stdout.clone()),
                None => (0, String::new()),
            }
        };
        Ok(crate::testing::command::outcome(spec, code, &stdout))
    }
}
