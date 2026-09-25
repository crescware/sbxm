//! Sandboxの中で走らせたcommandが返す応答。
//!
//! argvの並びは長いため、答える対象ごとにmatchを分ける。どのmatchにも一致しない起動は
//! 成功として扱う。実物のSandboxでも、sbxmが観測しない起動の結果は工程を左右しない。

use std::fmt::Write as _;

use crate::support::tools;
use crate::testing::value::COMMIT;

use super::World;

/// 成功と、対象が無いことを示す終了status。
const OK: (i32, String) = (0, String::new());

fn ok() -> (i32, String) {
    OK
}

fn missing() -> (i32, String) {
    (1, String::new())
}

impl World {
    /// `sbx exec [--user root] <name> -- <argv>`のargvを実行する。
    pub fn sandbox_exec(&self, args: &[&str]) -> (i32, String) {
        let Some(position) = args.iter().position(|arg| *arg == "--") else {
            return ok();
        };
        let inner = &args[position + 1..];
        let sandbox = args[position - 1];

        // 実物と同じく、中で何かを動かせばSandboxは起動する。read-onlyのつもりの
        // 検査でも状態が変わることを、fakeでも同じ性質として持つ。
        for row in self.sandboxes.borrow_mut().iter_mut() {
            if row.name == sandbox {
                row.running = true;
            }
        }

        self.probe(inner, sandbox)
            .or_else(|| self.filesystem(inner))
            .or_else(|| self.settings_of(inner))
            .or_else(|| self.repository_of(inner))
            .or_else(|| self.worktree_of(inner))
            .unwrap_or_else(ok)
    }

    /// `sbx exec -i`のstdinで受け取ったbyte列を、宣言fileまたはbundleとして置く起動。
    ///
    /// 実物の手順と同じく、受け取ったbyte列のdigestが期待と一致した場合だけ置き、一致
    /// しなければ何も置かずに失敗する。
    pub fn receive(&self, args: &[&str], input: &[u8]) -> (i32, String) {
        let Some(position) = args.iter().position(|arg| *arg == "--") else {
            return missing();
        };
        match &args[position + 1..] {
            ["sh", "-c", _script, "sh", destination, _, digest]
            | ["sh", "-c", _script, "sh", destination, digest] => {
                if crate::hash::sha256_hex(input) != *digest {
                    return (65, String::new());
                }
                self.place(Some((*digest).to_string()), destination);
                self.contents
                    .borrow_mut()
                    .insert((*destination).to_string(), input.to_vec());
                ok()
            }
            _ => missing(),
        }
    }

    /// Sandboxの状態を訊くだけの起動。
    fn probe(&self, inner: &[&str], _sandbox: &str) -> Option<(i32, String)> {
        match inner {
            // login shellが読むtoken環境変数file。sbxmが書いた内容をそのまま返す。
            ["sh", "-c", script, "sh", path] if script.contains("exec cat") => {
                Some(match self.settings.borrow().get(*path) {
                    Some(content) => (0, content.clone()),
                    // token環境変数fileのprobeが不在を示す専用status。
                    None => (44, String::new()),
                })
            }
            // sbxmがそのfileを書く。argvで渡った行をそのまま持つ。
            ["sh", "-c", script, "sh", first, second, third, path] if script.contains("printf") => {
                self.settings
                    .borrow_mut()
                    .insert((*path).to_string(), format!("{first}\n{second}\n{third}\n"));
                Some(ok())
            }
            // fetchの前の認証確認。登録があればGitHubは受け付ける。
            ["sh", "-c", script, "sh", _] if script.contains("ls-remote") => {
                Some(if self.secrets.borrow().is_empty() {
                    (
                        128,
                        "remote: Invalid username or token.\nfatal: Authentication failed\n"
                            .to_string(),
                    )
                } else {
                    ok()
                })
            }
            // Sandboxが持っているtoolを一度に答える。
            ["sh", "-c", script] if *script == tools::probe() => {
                let carried = self.commands.borrow();
                let mut listed = String::new();
                for name in carried.iter() {
                    // Stringへの書き込みは失敗しない。
                    let _ = writeln!(listed, "{name}");
                }
                Some((0, listed))
            }
            // 実物と同じく、SSH Agentは届かない。`printenv`は未設定を`1`で示す。
            ["printenv", "SSH_AUTH_SOCK"] => Some(missing()),
            ["ssh-add", "-L"] => Some((crate::support::sandbox::SSH_ADD_NO_AGENT, String::new())),
            _ => None,
        }
    }

    /// Sandbox内のfileを見る、または動かす起動。
    fn filesystem(&self, inner: &[&str]) -> Option<(i32, String)> {
        match inner {
            ["df", "-Pk", "/"] => Some((
                0,
                "Filesystem     1024-blocks      Used Available Capacity Mounted on\noverlay          20466256  14502976   4898320       75% /\n"
                    .to_string(),
            )),
            ["test", flag, path] => {
                // 模したSandboxにsymlinkは存在しない。
                let known = *flag != "-h" && self.present.borrow().contains(*path);
                Some(if known { ok() } else { missing() })
            }
            ["mkdir", "-p", path] | ["install", "-d", .., path] => {
                self.present.borrow_mut().insert((*path).to_string());
                Some(ok())
            }
            ["cat", "--", path] => Some(match self.contents.borrow().get(*path) {
                Some(contents) => (0, String::from_utf8_lossy(contents).into_owned()),
                None => missing(),
            }),
            ["sha256sum", path] => Some(match self.digests.borrow().get(*path) {
                Some(digest) => (0, format!("{digest}  {path}\n")),
                None => missing(),
            }),
            ["install", .., source, target] => {
                let digest = self.digests.borrow().get(*source).cloned();
                self.place(digest, target);
                Some(ok())
            }
            ["mv", "-f", source, target] => {
                let digest = self.digests.borrow_mut().remove(*source);
                self.present.borrow_mut().remove(*source);
                self.place(digest, target);
                Some(ok())
            }
            ["rm", "-f", rest @ ..] => {
                for path in rest {
                    self.present.borrow_mut().remove(*path);
                    self.digests.borrow_mut().remove(*path);
                }
                Some(ok())
            }
            _ => None,
        }
    }

    /// 中身のあるfileだけが行き先に現れる。
    fn place(&self, digest: Option<String>, target: &str) {
        if let Some(digest) = digest {
            self.present.borrow_mut().insert(target.to_string());
            self.digests.borrow_mut().insert(target.to_string(), digest);
        }
    }

    /// gitとghの利用者設定を読み書きする起動。
    fn settings_of(&self, inner: &[&str]) -> Option<(i32, String)> {
        match inner {
            ["git", "config", "--global", "--get", key] | ["gh", "config", "get", key, ..] => {
                Some(match self.settings.borrow().get(*key) {
                    Some(value) => (0, format!("{value}\n")),
                    None => missing(),
                })
            }
            ["git", "config", "--global", key, value] | ["gh", "config", "set", key, value, ..] => {
                self.settings
                    .borrow_mut()
                    .insert((*key).to_string(), (*value).to_string());
                Some(ok())
            }
            _ => None,
        }
    }

    /// bare repositoryそのものを触る起動。
    fn repository_of(&self, inner: &[&str]) -> Option<(i32, String)> {
        match inner {
            ["git", "init", "--bare", git_dir] => {
                self.present.borrow_mut().insert((*git_dir).to_string());
                *self.bare_git_dir.borrow_mut() = Some((*git_dir).to_string());
                Some(ok())
            }
            ["git", "--git-dir", _, "remote", "add", "origin", url] => {
                self.repository
                    .borrow_mut()
                    .insert("remote.origin.url".to_string(), (*url).to_string());
                Some(ok())
            }
            ["git", "--git-dir", _, "config", "--get-all", key] => {
                Some(match self.repository.borrow().get(*key) {
                    Some(value) => (0, format!("{value}\n")),
                    None => missing(),
                })
            }
            ["git", "--git-dir", _, "config", key, value] => {
                self.repository
                    .borrow_mut()
                    .insert((*key).to_string(), (*value).to_string());
                Some(ok())
            }
            ["git", "--git-dir", _, "rev-parse", "--is-bare-repository"] => {
                Some((0, "true\n".to_string()))
            }
            ["git", "--git-dir", _, "fsck", "--connectivity-only"]
            | ["git", "--git-dir", _, "fetch", "--prune", "origin"]
            | ["git", "check-ref-format", "--branch", _]
            | ["git", "--git-dir", _, "for-each-ref", "--format=%(refname)"] => Some(ok()),
            ["git", "--git-dir", _, "count-objects", "-v"] => {
                Some((0, "count: 0\nin-pack: 0\n".to_string()))
            }
            [
                "git",
                "--git-dir",
                _,
                "ls-remote",
                "--symref",
                "origin",
                "HEAD",
            ] => Some((
                0,
                format!("ref: refs/heads/{}\tHEAD\n", self.default_branch),
            )),
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
                Some(if reference.starts_with("refs/remotes/origin/") {
                    ok()
                } else {
                    missing()
                })
            }
            // hostから戻したbranchの先端も、模したhostはoriginと同じcommitに保存している。
            ["git", "--git-dir", _, "rev-parse", _]
            | ["git", "--git-dir", _, "rev-parse", "--verify", _] => {
                Some((0, format!("{COMMIT}\n")))
            }
            _ => None,
        }
    }

    /// managed worktreeを作る、または見る起動。
    fn worktree_of(&self, inner: &[&str]) -> Option<(i32, String)> {
        match inner {
            ["git", "--git-dir", _, "worktree", "add", rest @ ..] => {
                let branch = rest
                    .iter()
                    .position(|arg| *arg == "-b")
                    .and_then(|index| rest.get(index + 1))
                    .map(|value| (*value).to_string());
                // pathを読めない起動は、実物のgitと同じく使い方の誤りとして断る。
                let path = rest.iter().find(|arg| arg.contains(".tree-"))?;
                self.present.borrow_mut().insert((*path).to_string());
                self.worktrees
                    .borrow_mut()
                    .insert((*path).to_string(), branch);
                Some(ok())
            }
            [
                "git",
                "-C",
                _,
                "rev-parse",
                "--path-format=absolute",
                "--git-common-dir",
            ] => {
                let git_dir = self.bare_git_dir.borrow().clone().unwrap_or_default();
                Some((0, format!("{git_dir}\n")))
            }
            // `-z`はfieldごとにNUL、recordごとに空fieldを出す。bare repositoryが最初の
            // recordになり、作成済みのmanaged worktreeがそれに続く。
            [
                "git",
                "--git-dir",
                git_dir,
                "worktree",
                "list",
                "--porcelain",
                "-z",
            ] => {
                let bare_root = git_dir.strip_suffix("/.git").unwrap_or(git_dir);
                let mut listed = format!("worktree {bare_root}\0bare\0\0");
                for (path, branch) in self.worktrees.borrow().iter() {
                    let state = match branch {
                        Some(branch) => format!("branch refs/heads/{branch}"),
                        None => "detached".to_string(),
                    };
                    // Stringへの書き込みは失敗しない。
                    let _ = write!(listed, "worktree {path}\0{state}\0\0");
                }
                Some((0, listed))
            }
            ["git", "-C", _, "rev-parse", "HEAD"] => Some((0, format!("{COMMIT}\n"))),
            ["git", "-C", path, "symbolic-ref", "-q", "HEAD"] => {
                Some(match self.worktrees.borrow().get(*path) {
                    Some(Some(branch)) => (0, format!("refs/heads/{branch}\n")),
                    // detachedのworktreeはsymbolic refを持たない。
                    _ => missing(),
                })
            }
            _ => None,
        }
    }
}
