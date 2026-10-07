use crate::boundary::host::{CommandOutcome, HostEnvironment};
use crate::design::ProgressSink;
use crate::diagnostics::{Msg, Result};
use crate::msg;

use crate::support::sandbox;

/// 1つのworktreeの読み取りと、待ち時間の前に出す工程表示。
///
/// status、構築・修復、破壊操作で同じcommandを使う。clean/dirtyの表示、削除の拒否条件、
/// detached HEADの解釈などは呼び出し側が決め、ここでは観測結果だけを返す。
pub struct Inspection<'a> {
    host: &'a dyn HostEnvironment,
    sandbox: &'a str,
    path: &'a str,
    current: usize,
    total: usize,
    progress: &'a mut dyn ProgressSink,
}

impl<'a> Inspection<'a> {
    pub fn new(
        host: &'a dyn HostEnvironment,
        sandbox: &'a str,
        path: &'a str,
        current: usize,
        total: usize,
        progress: &'a mut dyn ProgressSink,
    ) -> Self {
        Self {
            host,
            sandbox,
            path,
            current,
            total,
            progress,
        }
    }

    pub fn changes(&mut self) -> Result<CommandOutcome> {
        self.step(msg!("progress-inspect-worktree-changes"));
        self.exec(&["status", "--porcelain=v2", "-z", "--untracked-files=all"])
    }

    pub fn ignored(&mut self) -> Result<CommandOutcome> {
        self.step(msg!("progress-inspect-worktree-ignored"));
        self.exec(&["status", "--porcelain=v2", "-z", "--ignored=traditional"])
    }

    pub fn head(&mut self) -> Result<String> {
        let outcome = self.head_outcome()?.require_success()?;
        Ok(outcome.stdout_text().trim().to_string())
    }

    pub fn head_outcome(&mut self) -> Result<CommandOutcome> {
        self.step(msg!("progress-inspect-worktree-head"));
        self.exec(&["rev-parse", "HEAD"])
    }

    pub fn path(&self) -> &str {
        self.path
    }

    pub fn preparing(&mut self) {
        self.step(msg!("progress-inspect-worktree-preparation"));
    }

    pub fn common_dir(&mut self) -> Result<String> {
        self.step(msg!("progress-inspect-worktree-repository"));
        let outcome = self
            .exec(&["rev-parse", "--path-format=absolute", "--git-common-dir"])?
            .require_success()?;
        Ok(outcome.stdout_text().trim().to_string())
    }

    pub fn branch_reference(&self) -> Result<CommandOutcome> {
        self.exec(&["symbolic-ref", "-q", "HEAD"])
    }

    pub fn branch(&self) -> Result<CommandOutcome> {
        self.exec(&["symbolic-ref", "--quiet", "--short", "HEAD"])
    }

    pub fn upstream(&self) -> Result<CommandOutcome> {
        self.exec(&["rev-parse", "--symbolic-full-name", "@{upstream}"])
    }

    pub fn git_dir(&mut self) -> Result<String> {
        self.step(msg!("progress-inspect-worktree-operations"));
        let outcome = self.exec(&["rev-parse", "--git-dir"])?.require_success()?;
        Ok(outcome.stdout_text().trim().to_string())
    }

    pub fn operation_marker(&self, git_dir: &str, marker: &str) -> Result<CommandOutcome> {
        let path = format!("{git_dir}/{marker}");
        sandbox::exec(self.host, self.sandbox, &["test", "-e", &path])
    }

    fn step(&mut self, message: Msg) {
        // worktree名に改行やterminal制御文字があっても、工程表示を1行に保つ。
        self.progress.step(
            message
                .with("path", format!("{:?}", self.path))
                .with("current", self.current)
                .with("total", self.total),
        );
    }

    fn exec(&self, args: &[&str]) -> Result<CommandOutcome> {
        let mut command = vec!["git", "-C", self.path];
        command.extend_from_slice(args);
        sandbox::exec(self.host, self.sandbox, &command)
    }
}
