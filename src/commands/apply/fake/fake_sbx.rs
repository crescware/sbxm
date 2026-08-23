use crate::testing::outcome::Checked;

use std::cell::RefCell;
use std::path::PathBuf;

use crate::boundary::host::{CommandOutcome, CommandSpec, HostEnvironment};
use crate::diagnostics::Result;
use crate::metadata::MAX_WORKTREES;
use crate::paths::{PRIVATE_FILE_MODE, PathScope};
use crate::project::SandboxLayout;
use crate::testing::sandbox::InnerCommandSandbox;
use crate::testing::value::COMMIT;

use super::canonical;

pub struct FakeSbx {
    pub listing: String,
    pub inner: InnerCommandSandbox,
    /// 外部commandを呼ぶ時点でproject lockを取れてしまうか。
    pub lock_path: Option<PathBuf>,
    pub lock_was_free: RefCell<Option<bool>>,
}

impl FakeSbx {
    pub fn listing(output: &str) -> FakeSbx {
        FakeSbx {
            listing: output.to_string(),
            inner: InnerCommandSandbox::new(),
            lock_path: None,
            lock_was_free: RefCell::new(None),
        }
    }

    pub fn answering(mut self, command: &str, stdout: &str) -> FakeSbx {
        self.inner = self.inner.answering(command, stdout);
        self
    }

    pub fn failing(mut self, command: &str) -> FakeSbx {
        self.inner = self.inner.failing(command);
        self
    }

    pub fn holding(mut self, paths: &[&str]) -> FakeSbx {
        self.inner = self.inner.holding(paths);
        self
    }

    /// 共有repositoryとworktreeが揃ったSandboxとして答える。
    pub fn holding_repository(self) -> Checked<FakeSbx> {
        let layout = SandboxLayout::new(&canonical()?);
        let git_dir = layout.bare_git_dir();
        let host = self
            .answering(
                &format!("git --git-dir {git_dir} rev-parse --is-bare-repository"),
                "true\n",
            )
            .answering(
                &format!("git --git-dir {git_dir} config --get-all remote.origin.url"),
                "https://github.com/Example-Org/Example-Repo.git\n",
            )
            .answering(
                &format!("git --git-dir {git_dir} config --get-all remote.origin.fetch"),
                "+refs/heads/*:refs/remotes/origin/*\n",
            )
            .answering(
                &format!("git --git-dir {git_dir} rev-parse refs/remotes/origin/main"),
                &format!("{COMMIT}\n"),
            );
        let mut host = host;
        for index in 0..MAX_WORKTREES {
            let path = layout.worktree(index);
            host = host.answering(
                &format!("git -C {path} rev-parse HEAD"),
                &format!("{COMMIT}\n"),
            );
            host = host.answering(
                &format!("git -C {path} rev-parse --path-format=absolute --git-common-dir"),
                &format!("{}\n", layout.bare_git_dir()),
            );
            // 案件はattached modeであり、branchを持てるのは最初の1本だけである。
            host = match index {
                0 => host.answering(
                    &format!("git -C {path} symbolic-ref -q HEAD"),
                    "refs/heads/main\n",
                ),
                _ => host.failing(&format!("git -C {path} symbolic-ref -q HEAD")),
            };
        }
        Ok(host.holding(&[&git_dir]))
    }

    /// workflowの実行中にlockが保持されているかを観測する。
    pub fn watching_lock(mut self, path: PathBuf) -> FakeSbx {
        self.lock_path = Some(path);
        self
    }

    pub fn calls(&self) -> Vec<Vec<String>> {
        self.inner.calls()
    }

    pub fn ran(&self, needle: &str) -> bool {
        self.inner.ran(needle)
    }
}

impl HostEnvironment for FakeSbx {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        if let Some(path) = &self.lock_path
            && self.lock_was_free.borrow().is_none()
        {
            let taken = crate::paths::acquire_exclusive_lock(
                path,
                std::time::Duration::from_millis(50),
                PRIVATE_FILE_MODE,
                PathScope::ProjectPath,
            );
            *self.lock_was_free.borrow_mut() = Some(taken.is_ok());
        }
        let outcome = self.inner.run(spec)?;
        // Sandbox一覧だけはこちらが答える。残りはinner commandへの応答に任せる。
        match spec.args.first() {
            Some(arg) if arg == "ls" => {
                Ok(crate::testing::command::outcome(spec, 0, &self.listing))
            }
            _ => Ok(outcome),
        }
    }
}
