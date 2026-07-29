//! `prepare`のtestが動かすSandbox世界のfake。
//!
//! 工程は外部commandの応答だけで決まるため、応答と観測できる状態をここが持つ。

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::command::{CommandOutcome, TimeoutClass};
use crate::config::{GitIdentity, GlobalConfig};
use crate::error::Result;
use crate::hash::sha256_hex;
use crate::i18n::Locale;
use crate::metadata::{self, ProjectMetadata};

use super::run::{PrepareOutput, run};
use crate::commands::add::run::AddRequest;
use crate::paths::{self, AbsoluteBasePath, PRIVATE_DIR_MODE, ProjectPaths};
use crate::project::ProjectId;
use crate::support::tools;
use crate::testing::archive::image_archive_bytes;
use crate::testing::value::COMMIT;

/// docker、`sbx`、gitの応答を状態として持ち、`add`と`prepare`の全工程を通せるhost。
///
/// 各工程の副作用は、その工程が成功したときにだけ起こす。中断した実行の続きを
/// 同じ`prepare`が進められるかどうかは、この性質の上で判定できる。
pub struct World {
    /// tag -> buildが宣言したlabel。
    pub images: RefCell<BTreeMap<String, Vec<(String, String)>>>,
    /// Template名 -> 対応するimage ID。
    pub templates: RefCell<BTreeMap<String, String>>,
    pub sandboxes: RefCell<Vec<SandboxRow>>,
    /// 登録済みcustom secretの対象host。
    pub secrets: RefCell<Vec<String>>,
    /// Sandbox内に存在するpath。
    pub present: RefCell<BTreeSet<String>>,
    /// Sandbox内のfileのdigest。
    pub digests: RefCell<BTreeMap<String, String>>,
    /// Sandbox内のgitとghの設定。
    pub settings: RefCell<BTreeMap<String, String>>,
    /// bare repositoryの設定値。
    pub repository: RefCell<BTreeMap<String, String>>,
    /// managed worktreeのpath -> branch。detachedは`None`。
    pub worktrees: RefCell<BTreeMap<String, Option<String>>>,
    /// Sandbox内にあるcommand。既定のtemplateが入れるものを持つ。
    pub commands: RefCell<BTreeSet<String>>,
    pub default_branch: String,
    /// 一致した起動を、実行せずにこのexit statusと標準出力で答える。副作用は起こさない。
    pub answer: RefCell<Option<(String, i32, String)>>,
    pub calls: RefCell<Vec<crate::command::CommandSpec>>,
}

#[derive(Clone)]
pub struct SandboxRow {
    pub name: String,
    pub workspace: String,
    pub template: String,
    /// 作成時にcustom secretが登録済みだったか。実物と同じく、あとから登録しても
    /// 既に存在するSandboxへはplaceholderが届かない。
    pub placeholder: bool,
}

pub const IMAGE_ID: &str =
    "sha256:3333333333333333333333333333333333333333333333333333333333333333";

impl World {
    pub fn new() -> World {
        World {
            images: RefCell::new(BTreeMap::new()),
            templates: RefCell::new(BTreeMap::new()),
            sandboxes: RefCell::new(Vec::new()),
            secrets: RefCell::new(
                crate::support::secret::GITHUB_HOSTS
                    .iter()
                    .map(|host| host.to_string())
                    .collect(),
            ),
            present: RefCell::new(BTreeSet::new()),
            digests: RefCell::new(BTreeMap::new()),
            settings: RefCell::new(BTreeMap::new()),
            repository: RefCell::new(BTreeMap::new()),
            worktrees: RefCell::new(BTreeMap::new()),
            commands: RefCell::new(
                tools::TOOLS
                    .iter()
                    .map(|tool| tool.name().to_string())
                    .collect(),
            ),
            default_branch: "main".to_string(),
            answer: RefCell::new(None),
            calls: RefCell::new(Vec::new()),
        }
    }

    /// 利用者がDockerfileから外したtoolを持たないSandbox。
    pub fn without(&self, program: &str) {
        self.commands.borrow_mut().remove(program);
    }

    /// Sandbox内に既にあるfile。cloneした案件が持ち込むものを表す。
    pub fn carrying(&self, path: &str) {
        self.present.borrow_mut().insert(path.to_string());
    }

    /// 次の実行で、指定した起動だけを失敗させる。
    pub fn failing(&self, needle: &str) {
        self.answering(needle, 1, "");
    }

    /// 失敗しながら出力も返す起動。実物と同じく、失敗は出力の空さでは見分けられない。
    pub fn failing_with(&self, needle: &str, stdout: &str) {
        self.answering(needle, 1, stdout);
    }

    /// 成功しながら何も出力しない起動。exit statusだけでは観測できたと言えない。
    pub fn succeeding_silently(&self, needle: &str) {
        self.answering(needle, 0, "");
    }

    pub fn answering(&self, needle: &str, code: i32, stdout: &str) {
        *self.answer.borrow_mut() = Some((needle.to_string(), code, stdout.to_string()));
    }

    pub fn nothing_fails(&self) {
        *self.answer.borrow_mut() = None;
    }

    pub fn invocations(&self) -> Vec<String> {
        self.calls
            .borrow()
            .iter()
            .map(|spec| format!("{} {}", spec.program, spec.args.join(" ")))
            .collect()
    }

    pub fn ran(&self, needle: &str) -> bool {
        self.invocations().iter().any(|call| call.contains(needle))
    }

    /// ここまでの起動数。以降の起動だけを見るために使う。
    pub fn mark(&self) -> usize {
        self.calls.borrow().len()
    }

    pub fn since(&self, mark: usize) -> Vec<String> {
        self.invocations().split_off(mark)
    }

    pub fn policy_of(&self, needle: &str) -> Option<(crate::command::OutputPolicy, TimeoutClass)> {
        self.calls
            .borrow()
            .iter()
            .find(|spec| format!("{} {}", spec.program, spec.args.join(" ")).contains(needle))
            .map(|spec| (spec.output, spec.timeout))
    }

    pub fn outcome(
        &self,
        spec: &crate::command::CommandSpec,
        code: i32,
        stdout: &str,
    ) -> CommandOutcome {
        crate::testing::command::outcome(spec, code, stdout)
    }

    pub fn host_git(&self, spec: &crate::command::CommandSpec) -> (i32, String) {
        let args: Vec<&str> = spec.args.iter().map(String::as_str).collect();
        match args.as_slice() {
            ["clone", _, target] => {
                // cloneが成功したときだけ、working treeができる。
                fs::create_dir_all(Path::new(target).join(".git")).expect("create the clone");
                (0, String::new())
            }
            ["rev-parse", "--is-bare-repository"] => (0, "false\n".to_string()),
            ["rev-parse", "--show-toplevel"] => (
                0,
                format!("{}\n", paths::display(spec.working_dir.as_ref().unwrap())),
            ),
            ["config", "--get-all", "remote.origin.url"] => (
                0,
                "git@github.com:Example-Org/Example-Repo.git\n".to_string(),
            ),
            _ => (0, String::new()),
        }
    }

    pub fn docker(&self, spec: &crate::command::CommandSpec) -> (i32, String) {
        let args: Vec<&str> = spec.args.iter().map(String::as_str).collect();
        match args.as_slice() {
            ["build", rest @ ..] => {
                let mut labels = Vec::new();
                let mut tag = String::new();
                let mut index = 0;
                while index < rest.len() {
                    match rest[index] {
                        "--label" => {
                            if let Some((key, value)) = rest[index + 1].split_once('=') {
                                labels.push((key.to_string(), value.to_string()));
                            }
                            index += 2;
                        }
                        "--tag" => {
                            tag = rest[index + 1].to_string();
                            index += 2;
                        }
                        _ => index += 1,
                    }
                }
                self.images.borrow_mut().insert(tag, labels);
                (0, String::new())
            }
            ["image", "ls", "--quiet", name] => (
                0,
                if self.images.borrow().contains_key(*name) {
                    "0123456789ab\n".to_string()
                } else {
                    String::new()
                },
            ),
            ["image", "inspect", name] => match self.images.borrow().get(*name) {
                Some(labels) => {
                    let rendered = labels
                        .iter()
                        .map(|(key, value)| format!("\"{key}\":\"{value}\""))
                        .collect::<Vec<_>>()
                        .join(",");
                    (
                        0,
                        format!(r#"[{{"Id":"{IMAGE_ID}","Config":{{"Labels":{{{rendered}}}}}}}]"#),
                    )
                }
                None => (1, String::new()),
            },
            ["image", "save", name, "--output", output] => {
                let owned = self.images.borrow().get(*name).cloned().unwrap_or_default();
                let labels: Vec<(&str, &str)> = owned
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str()))
                    .collect();
                fs::write(output, image_archive_bytes(name, IMAGE_ID, &labels))
                    .expect("write the archive");
                (0, String::new())
            }
            _ => (0, String::new()),
        }
    }

    pub fn sbx(&self, spec: &crate::command::CommandSpec) -> (i32, String) {
        let args: Vec<&str> = spec.args.iter().map(String::as_str).collect();
        match args.as_slice() {
            ["ls", "--json"] => {
                let rendered = self
                    .sandboxes
                    .borrow()
                    .iter()
                    .map(|row| {
                        format!(
                            r#"{{"name":"{}","state":"running","workspace":"{}","template":"{}","active_sessions":0}}"#,
                            row.name, row.workspace, row.template
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                (0, format!("[{rendered}]"))
            }
            ["template", "ls", "--json"] => {
                // runtimeのimage storeはrepositoryとtagで示し、prefixを補う。
                let rendered = self
                        .templates
                        .borrow()
                        .iter()
                        .map(|(name, id)| {
                            let (repository, tag) =
                                name.rsplit_once(':').expect("an image reference");
                            format!(
                                r#"{{"id":"{id}","repository":"docker.io/library/{repository}","tag":"{tag}"}}"#
                            )
                        })
                        .collect::<Vec<_>>()
                        .join(",");
                (0, format!(r#"{{"images":[{rendered}]}}"#))
            }
            ["template", "load", archive] => {
                let manifest = crate::archive::read_manifest(Path::new(archive))
                    .expect("the archive names the image it holds");
                self.templates.borrow_mut().insert(
                    manifest.repo_tags[0].clone(),
                    manifest.config_digest.clone(),
                );
                (0, String::new())
            }
            [
                "create",
                "--name",
                name,
                "--template",
                template,
                _kit,
                workspace,
            ] => {
                let registered = self
                    .secrets
                    .borrow()
                    .iter()
                    .any(|target| target == crate::support::secret::GITHUB_HOST);
                self.sandboxes.borrow_mut().push(SandboxRow {
                    name: name.to_string(),
                    workspace: workspace.to_string(),
                    template: template.to_string(),
                    placeholder: registered,
                });
                (0, String::new())
            }
            ["secret", "ls", name] => {
                let secrets = self.secrets.borrow();
                if secrets.is_empty() {
                    return (0, format!("No secrets found for scope \"{name}\".\n"));
                }
                // 1件のcustom secretが複数hostを覆う。TARGETS列は空白1つで並ぶ。
                let mut table =
                    String::from("CUSTOM SECRETS\nSCOPE   TARGETS   ENV   PLACEHOLDER   SECRET\n");
                table.push_str(&format!(
                    "{name}   {}   GH_TOKEN   sbx-cs-example   ghp_example\n",
                    secrets.join(" ")
                ));
                (0, table)
            }
            ["cp", "--follow-link", source, target] => {
                let digest = sha256_hex(&fs::read(source).expect("read the declared file"));
                let path = target
                    .split_once(':')
                    .expect("a sandbox path")
                    .1
                    .to_string();
                self.present.borrow_mut().insert(path.clone());
                self.digests.borrow_mut().insert(path, digest);
                (0, String::new())
            }
            ["daemon", ..] => (0, String::new()),
            _ => self.sandbox_exec(&args),
        }
    }

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

impl crate::command::HostEnvironment for World {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &crate::command::CommandSpec) -> Result<CommandOutcome> {
        self.calls.borrow_mut().push(spec.clone());
        let invocation = format!("{} {}", spec.program, spec.args.join(" "));
        if let Some((needle, code, stdout)) = self.answer.borrow().as_ref()
            && invocation.contains(needle.as_str())
        {
            // 答えを差し替えた工程は実行せず、その工程の副作用も残さない。
            return Ok(self.outcome(spec, *code, stdout));
        }

        let (code, stdout) = match spec.program.as_str() {
            "git" => self.host_git(spec),
            "docker" => self.docker(spec),
            "sbx" => self.sbx(spec),
            _ => (0, String::new()),
        };
        Ok(self.outcome(spec, code, &stdout))
    }
}

/// 宣言file 1件を持つ、実行時と同じ形の入力一式。
pub struct Bench {
    pub _base: tempfile::TempDir,
    pub _home: tempfile::TempDir,
    pub workspace_root: tempfile::TempDir,
    pub config: GlobalConfig,
}

pub fn bench() -> Bench {
    let base = tempfile::tempdir().expect("temporary base path");
    let home = tempfile::tempdir().expect("temporary home");
    let workspace_root = tempfile::tempdir().expect("temporary workspace root");
    fs::set_permissions(
        workspace_root.path(),
        fs::Permissions::from_mode(PRIVATE_DIR_MODE),
    )
    .expect("the workspace root belongs to the current user only");

    let source = home.path().join("declared.toml");
    fs::write(&source, b"declared = true\n").expect("write the declared file");

    let config = GlobalConfig {
        language: Locale::En,
        base_path: AbsoluteBasePath::new(base.path()).expect("valid base path"),
        git: GitIdentity {
            user_name: "Example User".into(),
            user_email: "user@example.com".into(),
        },
        files: vec![crate::config::FileDeclaration {
            source: crate::config::HostFileSource::new(&paths::display(&source))
                .expect("valid source"),
            destination: crate::config::SandboxHomeRelativePath::new(".config/example/config.toml")
                .expect("valid destination"),
        }],
    };
    Bench {
        _base: base,
        _home: home,
        workspace_root,
        config,
    }
}

impl Bench {
    /// `add`で登録してから`prepare`で構築する。工程は通しで判定する。
    pub fn build(&self, world: &World, request: &AddRequest) -> Result<PrepareOutput> {
        crate::commands::add::run::run(&self.config, request, world)?;
        run(
            &self.config,
            &request.project,
            world,
            self.workspace_root.path(),
        )
    }

    pub fn stored(&self, project: &str) -> ProjectMetadata {
        let canonical = ProjectId::parse(project).unwrap().canonical();
        let paths = ProjectPaths::derive(&self.config.base_path, &canonical);
        metadata::load(&paths)
            .expect("read the metadata")
            .expect("present")
    }
}
