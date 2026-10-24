use crate::testing::outcome::{Checked, Required};

use crate::boundary::host::{CommandOutcome, CommandSpec};
use crate::diagnostics::Result;
use crate::testing::host::AnsweredHost;
use crate::testing::inside::{frame_inside_check, split_inside_check};
use std::cell::RefCell;

/// Sandbox一覧を返し、実行された指定を記録するhost。
///
/// 応答のkeyは引数全体(`exec <name> -- printenv SSH_AUTH_SOCK`のような文字列)である。
/// `--`より後ろのinner commandだけで答えるfakeは
/// `crate::testing::sandbox::InnerCommandSandbox`。
///
/// まとめた確認（Sandboxの中のcommandを1回の`sbx exec`で走らせる手順）には、中の
/// commandごとに`exec <name> -- <command>`の応答を引き、実物と同じ形に並べて答える。
pub struct FakeSbx {
    pub listing: RefCell<Vec<String>>,
    pub answers: std::collections::HashMap<String, (i32, String)>,
    /// 同じ指定へ順に返す応答。末尾から取り出す。
    pub sequences: RefCell<std::collections::HashMap<String, Vec<(i32, String)>>>,
    pub specs: RefCell<Vec<CommandSpec>>,
}

impl FakeSbx {
    pub fn listing(output: &str) -> FakeSbx {
        FakeSbx {
            listing: RefCell::new(vec![output.to_string()]),
            answers: std::collections::HashMap::new(),
            sequences: RefCell::new(std::collections::HashMap::new()),
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
                    .map(|value| (*value).to_string())
                    .collect(),
            ),
            answers: std::collections::HashMap::new(),
            sequences: RefCell::new(std::collections::HashMap::new()),
            specs: RefCell::new(Vec::new()),
        }
    }

    pub fn answering(mut self, command: &str, code: i32, stdout: &str) -> FakeSbx {
        self.answers
            .insert(command.to_string(), (code, stdout.to_string()));
        self
    }

    /// 1つの指定へ、呼び出しごとに異なる応答を返す。最後の1件は繰り返し使う。
    ///
    /// mutationの前後で観測が変わる工程を、実機と同じ順序で辿るために使う。
    pub fn answering_in_turn(self, command: &str, answers: &[(i32, &str)]) -> FakeSbx {
        self.sequences.borrow_mut().insert(
            command.to_string(),
            answers
                .iter()
                .rev()
                .map(|(code, stdout)| (*code, (*stdout).to_string()))
                .collect(),
        );
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

    pub fn removed_any(&self) -> bool {
        self.calls()
            .iter()
            .any(|args| super::is_sandbox_removal(args))
    }

    /// scoped secretを保持する削除要求で、このSandboxを消したか。
    ///
    /// `sbx rm`はscoped secretも消すため、1度でも呼べば消したことに数えない。
    pub fn removed(&self, name: &str) -> bool {
        let target = format!("http://localhost/sandbox/{name}?");
        let calls = self.calls();
        !calls
            .iter()
            .any(|args| args.first().is_some_and(|arg| arg == "rm"))
            && calls.iter().any(|args| {
                args.iter().any(|arg| arg == "DELETE")
                    && args.iter().any(|arg| {
                        arg.starts_with(&target) && arg.ends_with("&delete_scoped_secrets=false")
                    })
            })
    }

    /// このSandboxを消す起動を、形を問わず1度でも求めたか。
    pub fn removal_requested(&self, name: &str) -> bool {
        let target = format!("http://localhost/sandbox/{name}?");
        self.calls().iter().any(|args| {
            super::is_sandbox_removal(args)
                && args
                    .iter()
                    .any(|arg| arg == name || arg.starts_with(&target))
        })
    }

    /// 1つの指定への応答。順に返す応答があればそれを先に使う。
    fn lookup(&self, key: &str) -> Option<(i32, String)> {
        if let Some(queued) = self.sequences.borrow_mut().get_mut(key) {
            return Some(if queued.len() > 1 {
                queued.pop().unwrap_or_default()
            } else {
                queued.last().cloned().unwrap_or_default()
            });
        }
        self.answers.get(key).cloned()
    }

    /// 引数が一致した最後の1件の指定。envとoutput policyの検証に使う。
    pub fn spec(&self, needle: &str) -> Checked<CommandSpec> {
        Ok(self
            .specs
            .borrow()
            .iter()
            .rev()
            .find(|spec| spec.args.join(" ").contains(needle))
            .required_because(&format!("no command matched {needle}"))?
            .clone())
    }
}

impl AnsweredHost for FakeSbx {
    fn has_command(&self, _program: &str) -> bool {
        true
    }

    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.specs.borrow_mut().push(spec.clone());
        if spec.program == "sbx"
            && let Some((sandbox, _timed, commands)) = split_inside_check(&spec.args)
        {
            // 指定の無いcommandは、1件ずつ起動されたときと同じく、成功して何も出さない。
            let answers: Vec<(i32, String)> = commands
                .iter()
                .map(|command| {
                    self.lookup(&format!("exec {sandbox} -- {}", command.join(" ")))
                        .unwrap_or((0, String::new()))
                })
                .collect();
            return Ok(crate::testing::command::outcome(
                spec,
                0,
                &frame_inside_check(&answers),
            ));
        }
        let key = spec.args.join(" ");
        let (code, stdout) = if spec.args.first().is_some_and(|arg| arg == "ls") {
            let mut listings = self.listing.borrow_mut();
            let output = if listings.len() > 1 {
                listings.pop().unwrap_or_default()
            } else {
                listings.last().cloned().unwrap_or_default()
            };
            (0, output)
        } else if let Some(answer) = self.lookup(&key) {
            answer
        } else {
            match self.answers.get(&key) {
                Some((code, stdout)) => (*code, stdout.clone()),
                // 指定が無ければ、Templateは1つも無い。v0.48.0は0件でも空の配列を返す。
                None if spec.args == ["template", "ls", "--json"] => (0, "[]".to_string()),
                // 指定が無ければ、このbuildが求める最低versionのCLIである。
                None if spec.args == ["version"] => (0, super::supported_sbx_version()),
                // 指定が無ければ、hostのgitもこのbuildが求める最低versionである。
                None if spec.program == "git" && spec.args == ["--version"] => {
                    (0, super::supported_git_version())
                }
                None if spec.args == ["daemon", "status", "--json"] => (
                    0,
                    r#"{"status":"running","socket":"/tmp/sbxm-test-sandboxd.sock"}"#.to_string(),
                ),
                None if spec.program == "curl" && spec.args.iter().any(|arg| arg == "GET") => {
                    let name = spec
                        .args
                        .last()
                        .and_then(|url| url.rsplit('/').next())
                        .unwrap_or_default();
                    (0, serde_json::json!({"name":name,"id":"ec55cefe-9919-4c0e-952c-db88e5466db2"}).to_string())
                }
                // 保存の前にworktreeのHEADを置く手順は、指定が無ければ保存するものが
                // 無いと答える。実物は何も書かずに終わらないため、空の答えを返さない。
                None if spec
                    .args
                    .iter()
                    .any(|arg| arg == crate::support::host_sync::PLACE_SAVE_REFS) =>
                {
                    (0, "empty\n".to_string())
                }
                // 指定が無ければ、sbxが用意するRemote SSHの設定がある。
                None if spec.program == "ssh"
                    && spec.args.first().is_some_and(|arg| arg == "-G") =>
                {
                    (0, "proxycommand sbx ssh-proxy %h\n".to_string())
                }
                None => (0, String::new()),
            }
        };
        Ok(crate::testing::command::outcome(spec, code, &stdout))
    }
}
