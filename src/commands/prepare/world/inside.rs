//! Sandboxの中で走らせたcommandが返す応答。

use crate::support::tools;
use crate::testing::value::COMMIT;

use super::World;

impl World {
    /// `sbx exec [--user root] <name> -- <argv>`のargvを実行する。
    pub fn sandbox_exec(&self, args: &[&str]) -> (i32, String) {
        let Some(position) = args.iter().position(|arg| *arg == "--") else {
            return (0, String::new());
        };
        let inner = &args[position + 1..];
        let sandbox = args[position - 1];
        let missing = (1, String::new());
        let ok = (0, String::new());

        match inner {
            ["sh", "-c", script] if *script == crate::support::secret::placeholder_probe() => {
                let carried = self
                    .sandboxes
                    .borrow()
                    .iter()
                    .any(|row| row.name == sandbox && row.placeholder);
                if carried {
                    (0, "sbx-cs-example".to_string())
                } else {
                    ok
                }
            }
            // Sandboxが持っているtoolを一度に答える。
            ["sh", "-c", script] if *script == tools::probe() => {
                let carried = self.commands.borrow();
                (
                    0,
                    carried
                        .iter()
                        .map(|name| format!("{name}\n"))
                        .collect::<String>(),
                )
            }
            // 実物と同じく、SSH Agentは届かない。`printenv`は未設定を`1`で示す。
            ["printenv", "SSH_AUTH_SOCK"] => missing,
            ["ssh-add", "-L"] => (crate::support::sandbox::SSH_ADD_NO_AGENT, String::new()),
            ["test", flag, path] => {
                let known = match *flag {
                    // 模したSandboxにsymlinkは存在しない。
                    "-h" => false,
                    _ => self.present.borrow().contains(*path),
                };
                if known { ok } else { missing }
            }
            ["mkdir", "-p", path] => {
                self.present.borrow_mut().insert(path.to_string());
                ok
            }
            ["sha256sum", path] => match self.digests.borrow().get(*path) {
                Some(digest) => (0, format!("{digest}  {path}\n")),
                None => missing,
            },
            ["install", "-d", .., path] => {
                self.present.borrow_mut().insert(path.to_string());
                ok
            }
            ["install", .., source, target] => {
                let digest = self.digests.borrow().get(*source).cloned();
                if let Some(digest) = digest {
                    self.present.borrow_mut().insert(target.to_string());
                    self.digests.borrow_mut().insert(target.to_string(), digest);
                }
                ok
            }
            ["mv", "-f", source, target] => {
                let digest = self.digests.borrow_mut().remove(*source);
                self.present.borrow_mut().remove(*source);
                if let Some(digest) = digest {
                    self.present.borrow_mut().insert(target.to_string());
                    self.digests.borrow_mut().insert(target.to_string(), digest);
                }
                ok
            }
            ["rm", "-f", rest @ ..] => {
                for path in rest {
                    self.present.borrow_mut().remove(*path);
                    self.digests.borrow_mut().remove(*path);
                }
                ok
            }
            ["git", "config", "--global", "--get", key] => match self.settings.borrow().get(*key) {
                Some(value) => (0, format!("{value}\n")),
                None => missing,
            },
            ["git", "config", "--global", key, value] => {
                self.settings
                    .borrow_mut()
                    .insert(key.to_string(), value.to_string());
                ok
            }
            ["gh", "config", "get", key, ..] => match self.settings.borrow().get(*key) {
                Some(value) => (0, format!("{value}\n")),
                None => missing,
            },
            ["gh", "config", "set", key, value, ..] => {
                self.settings
                    .borrow_mut()
                    .insert(key.to_string(), value.to_string());
                ok
            }
            ["git", "init", "--bare", git_dir] => {
                self.present.borrow_mut().insert(git_dir.to_string());
                ok
            }
            ["git", "--git-dir", _, "remote", "add", "origin", url] => {
                self.repository
                    .borrow_mut()
                    .insert("remote.origin.url".to_string(), url.to_string());
                ok
            }
            ["git", "--git-dir", _, "config", "--get-all", key] => {
                match self.repository.borrow().get(*key) {
                    Some(value) => (0, format!("{value}\n")),
                    None => missing,
                }
            }
            ["git", "--git-dir", _, "config", key, value] => {
                self.repository
                    .borrow_mut()
                    .insert(key.to_string(), value.to_string());
                ok
            }
            ["git", "--git-dir", _, "rev-parse", "--is-bare-repository"] => {
                (0, "true\n".to_string())
            }
            ["git", "--git-dir", _, "fsck", "--connectivity-only"] => ok,
            ["git", "--git-dir", _, "fetch", "--prune", "origin"] => ok,
            [
                "git",
                "--git-dir",
                _,
                "ls-remote",
                "--symref",
                "origin",
                "HEAD",
            ] => (
                0,
                format!("ref: refs/heads/{}\tHEAD\n", self.default_branch),
            ),
            ["git", "check-ref-format", "--branch", _] => ok,
            [
                "git",
                "--git-dir",
                _,
                "show-ref",
                "--verify",
                "--quiet",
                reference,
            ] => {
                // 解決できないrefの扱いは、repository moduleのtestが固定する。
                if reference.starts_with("refs/remotes/origin/") {
                    ok
                } else {
                    missing
                }
            }
            ["git", "--git-dir", _, "rev-parse", _] => (0, format!("{COMMIT}\n")),
            ["git", "--git-dir", _, "worktree", "add", rest @ ..] => {
                let branch = rest
                    .iter()
                    .position(|arg| *arg == "-b")
                    .map(|index| rest[index + 1].to_string());
                let path = rest
                    .iter()
                    .find(|arg| arg.contains(".tree-"))
                    .expect("a worktree path")
                    .to_string();
                self.present.borrow_mut().insert(path.clone());
                self.worktrees.borrow_mut().insert(path, branch);
                ok
            }
            ["git", "-C", _, "rev-parse", "HEAD"] => (0, format!("{COMMIT}\n")),
            ["git", "-C", path, "symbolic-ref", "-q", "HEAD"] => {
                match self.worktrees.borrow().get(*path) {
                    Some(Some(branch)) => (0, format!("refs/heads/{branch}\n")),
                    // detachedのworktreeはsymbolic refを持たない。
                    _ => missing,
                }
            }
            _ => ok,
        }
    }
}
