use super::*;
use crate::command::{CommandOutcome, OutputPolicy, TimeoutClass};
use crate::compatibility::SandboxState;
use crate::config::GitIdentity;
use crate::error::Result;
use crate::hash::sha256_hex;
use crate::i18n::Locale;
use crate::paths::{self, AbsoluteBasePath, PRIVATE_DIR_MODE};
use crate::testing::add_request::request;
use crate::testing::archive::image_archive_bytes;
use crate::testing::value::COMMIT;
use crate::workflow::add::AddRequest;
use crate::workflow::files::Placement;
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::os::unix::fs::PermissionsExt;

#[test]
fn a_project_that_is_not_registered_is_sent_to_add() {
    let bench = bench();
    let world = World::new();

    let error = run(
        &bench.config,
        &ProjectId::parse("example-org/example-repo").expect("valid project id"),
        &world,
        bench.workspace_root.path(),
    )
    .expect_err("there is nothing to build yet");
    assert_eq!(error.first_id(), Some(ErrorId::ProjectNotManaged));

    let diagnostic = &error.diagnostics()[0];
    assert_eq!(
        diagnostic.remediation.as_ref().map(|message| message.id),
        Some("remediation-project-not-managed")
    );
    assert!(
        world.invocations().is_empty(),
        "nothing is asked of the host: {:?}",
        world.invocations()
    );
}

/// docker、`sbx`、gitの応答を状態として持ち、`add`と`prepare`の全工程を通せるhost。
///
/// 各工程の副作用は、その工程が成功したときにだけ起こす。中断した実行の続きを
/// 同じ`prepare`が進められるかどうかは、この性質の上で判定できる。
struct World {
    /// tag -> buildが宣言したlabel。
    images: RefCell<BTreeMap<String, Vec<(String, String)>>>,
    /// Template名 -> 対応するimage ID。
    templates: RefCell<BTreeMap<String, String>>,
    sandboxes: RefCell<Vec<SandboxRow>>,
    /// 登録済みcustom secretの対象host。
    secrets: RefCell<Vec<String>>,
    /// Sandbox内に存在するpath。
    present: RefCell<BTreeSet<String>>,
    /// Sandbox内のfileのdigest。
    digests: RefCell<BTreeMap<String, String>>,
    /// Sandbox内のgitとghの設定。
    settings: RefCell<BTreeMap<String, String>>,
    /// bare repositoryの設定値。
    repository: RefCell<BTreeMap<String, String>>,
    /// managed worktreeのpath -> branch。detachedは`None`。
    worktrees: RefCell<BTreeMap<String, Option<String>>>,
    /// Sandbox内にあるcommand。既定のtemplateが入れるものを持つ。
    commands: RefCell<BTreeSet<String>>,
    default_branch: String,
    /// 一致した起動を失敗させる。副作用は起こさない。
    fail: RefCell<Option<String>>,
    calls: RefCell<Vec<crate::command::CommandSpec>>,
}

#[derive(Clone)]
struct SandboxRow {
    name: String,
    workspace: String,
    template: String,
    /// 作成時にcustom secretが登録済みだったか。実物と同じく、あとから登録しても
    /// 既に存在するSandboxへはplaceholderが届かない。
    placeholder: bool,
}

const IMAGE_ID: &str = "sha256:3333333333333333333333333333333333333333333333333333333333333333";

impl World {
    fn new() -> World {
        World {
            images: RefCell::new(BTreeMap::new()),
            templates: RefCell::new(BTreeMap::new()),
            sandboxes: RefCell::new(Vec::new()),
            secrets: RefCell::new(
                crate::workflow::secret::GITHUB_HOSTS
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
            fail: RefCell::new(None),
            calls: RefCell::new(Vec::new()),
        }
    }

    /// 利用者がDockerfileから外したtoolを持たないSandbox。
    fn without(&self, program: &str) {
        self.commands.borrow_mut().remove(program);
    }

    /// Sandbox内に既にあるfile。cloneした案件が持ち込むものを表す。
    fn carrying(&self, path: &str) {
        self.present.borrow_mut().insert(path.to_string());
    }

    /// 次の実行で、指定した起動だけを失敗させる。
    fn failing(&self, needle: &str) {
        *self.fail.borrow_mut() = Some(needle.to_string());
    }

    fn nothing_fails(&self) {
        *self.fail.borrow_mut() = None;
    }

    fn invocations(&self) -> Vec<String> {
        self.calls
            .borrow()
            .iter()
            .map(|spec| format!("{} {}", spec.program, spec.args.join(" ")))
            .collect()
    }

    fn ran(&self, needle: &str) -> bool {
        self.invocations().iter().any(|call| call.contains(needle))
    }

    /// ここまでの起動数。以降の起動だけを見るために使う。
    fn mark(&self) -> usize {
        self.calls.borrow().len()
    }

    fn since(&self, mark: usize) -> Vec<String> {
        self.invocations().split_off(mark)
    }

    fn policy_of(&self, needle: &str) -> Option<(crate::command::OutputPolicy, TimeoutClass)> {
        self.calls
            .borrow()
            .iter()
            .find(|spec| format!("{} {}", spec.program, spec.args.join(" ")).contains(needle))
            .map(|spec| (spec.output, spec.timeout))
    }

    fn outcome(
        &self,
        spec: &crate::command::CommandSpec,
        code: i32,
        stdout: &str,
    ) -> CommandOutcome {
        crate::testing::command::outcome(spec, code, stdout)
    }

    fn host_git(&self, spec: &crate::command::CommandSpec) -> (i32, String) {
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

    fn docker(&self, spec: &crate::command::CommandSpec) -> (i32, String) {
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

    fn sbx(&self, spec: &crate::command::CommandSpec) -> (i32, String) {
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
                    .any(|target| target == crate::workflow::secret::GITHUB_HOST);
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
    fn sandbox_exec(&self, args: &[&str]) -> (i32, String) {
        let Some(position) = args.iter().position(|arg| *arg == "--") else {
            return (0, String::new());
        };
        let inner = &args[position + 1..];
        let sandbox = args[position - 1];
        let missing = (1, String::new());
        let ok = (0, String::new());

        match inner {
            ["sh", "-c", script] if *script == super::super::secret::placeholder_probe() => {
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
            ["ssh-add", "-L"] => (crate::workflow::sandbox::SSH_ADD_NO_AGENT, String::new()),
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
        if let Some(needle) = self.fail.borrow().as_deref()
            && invocation.contains(needle)
        {
            // 失敗した工程は、その工程の副作用を残さない。
            return Ok(self.outcome(spec, 1, ""));
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
struct Bench {
    _base: tempfile::TempDir,
    _home: tempfile::TempDir,
    workspace_root: tempfile::TempDir,
    config: GlobalConfig,
}

fn bench() -> Bench {
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
    fn build(&self, world: &World, request: &AddRequest) -> Result<PrepareOutput> {
        super::super::add::run(&self.config, request, world)?;
        run(
            &self.config,
            &request.project,
            world,
            self.workspace_root.path(),
        )
    }

    fn stored(&self, project: &str) -> ProjectMetadata {
        let canonical = ProjectId::parse(project).unwrap().canonical();
        let paths = ProjectPaths::derive(&self.config.base_path, &canonical);
        metadata::load(&paths)
            .expect("read the metadata")
            .expect("present")
    }
}

/// `add`と`prepare`が外部工程を呼ぶ順に並べた、失敗させる工程とその診断。
const STEPS: [(&str, ErrorId); 11] = [
    ("git clone git@github.com", ErrorId::ExternalCommandFailed),
    ("docker build", ErrorId::ExternalCommandFailed),
    ("docker image save", ErrorId::ExternalCommandFailed),
    ("sbx template load", ErrorId::ExternalCommandFailed),
    ("sbx create", ErrorId::ExternalCommandFailed),
    ("sbx cp --follow-link", ErrorId::ExternalCommandFailed),
    ("config --global user.name", ErrorId::ExternalCommandFailed),
    ("sbx secret ls", ErrorId::ExternalCommandFailed),
    ("git init --bare", ErrorId::ExternalCommandFailed),
    ("check-ref-format", ErrorId::InvalidBranchName),
    ("worktree add", ErrorId::ExternalCommandFailed),
];

#[test]
fn an_interruption_at_any_step_is_continued_by_the_same_prepare() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None);

    // 1工程ずつ後ろへずらして失敗させる。次の実行がそこまで進めることが継続の証拠になる。
    for (step, expected) in STEPS {
        world.failing(step);
        let error = bench
            .build(&world, &request)
            .expect_err("the run stops at the step that failed");
        assert_eq!(error.first_id(), Some(expected), "{step}");
        world.nothing_fails();
    }

    // 最後に失敗したのはworktree作成であり、続きの実行はそこから進む。
    let mark = world.mark();
    let output = bench
        .build(&world, &request)
        .expect("the same add finishes");
    let tail = world.since(mark);

    assert!(!output.already_built);
    assert_eq!(output.mode, CreationMode::Attached);
    assert_eq!(output.start_ref, "main");
    assert_eq!(output.sandbox_state, SandboxState::Running);
    assert_eq!(output.worktrees.len(), 1);
    assert_eq!(output.worktrees[0].path, "example-repo.tree-0");
    assert_eq!(output.worktrees[0].head.as_deref(), Some(COMMIT));
    assert_eq!(output.files.len(), 1);
    assert_eq!(
        output.files[0].placement,
        Placement::Unchanged,
        "an earlier run placed the file, and an identical destination is left alone"
    );

    let stored = bench.stored("Example-Org/Example-Repo");
    assert_eq!(stored.provisioning.start_ref.as_deref(), Some("main"));

    // 成功済みの成果物は作り直さない。
    for done in [
        "git clone git@github.com",
        "docker build",
        "sbx template load",
        "sbx create",
        "git init --bare",
    ] {
        assert!(
            !tail.iter().any(|call| call.contains(done)),
            "{done} was already done: {tail:?}"
        );
    }
    assert_eq!(
        tail.iter()
            .filter(|call| call.contains("worktree add"))
            .count(),
        1,
        "the run continues with the step that had failed: {tail:?}"
    );
    // archiveは工程へ到達するたびに作り直す。
    assert!(tail.iter().any(|call| call.contains("docker image save")));
}

#[test]
fn a_finished_build_is_a_no_op_for_the_same_add() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None);
    bench.build(&world, &request).expect("the first run builds");

    let mark = world.mark();
    let output = bench
        .build(&world, &request)
        .expect("the second run changes nothing");

    assert!(output.already_built);
    for forbidden in [
        "docker build",
        "docker image save",
        "sbx template load",
        "sbx create",
        "sbx daemon stop",
        "sbx cp",
        "worktree add",
        "git clone",
    ] {
        assert!(
            !world
                .since(mark)
                .iter()
                .any(|call| call.contains(forbidden)),
            "a finished project must not run {forbidden}: {:?}",
            world.since(mark)
        );
    }
}

#[test]
fn git_is_given_the_placeholder_before_it_reaches_github() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None);
    bench.build(&world, &request).expect("the build completes");

    let calls = world.invocations();
    let position = |needle: &str| {
        calls
            .iter()
            .position(|call| call.contains(needle))
            .unwrap_or_else(|| panic!("no command matched {needle}: {calls:?}"))
    };
    assert!(
        position("credential.https://github.com.helper") < position("fetch --prune origin"),
        "a fetch without the credential asks for a username and never finishes"
    );
}

#[test]
fn a_missing_secret_stops_the_build_and_the_same_add_continues_once_it_is_there() {
    let bench = bench();
    let world = World::new();
    world.secrets.borrow_mut().clear();
    let request = request("Example-Org/Example-Repo", None, None);

    let error = bench
        .build(&world, &request)
        .expect_err("a build without repository access cannot continue");
    assert_eq!(error.first_id(), Some(ErrorId::GithubSecretMissing));
    assert!(
        !world.ran("git init --bare"),
        "the sandbox repository is not made without the secret"
    );
    // custom secretはSandboxの作成時に結び付く。先に作ってしまうと、登録しても
    // placeholderの届かないSandboxが残り、作り直しを強いることになる。
    assert!(
        !world.ran("sbx create"),
        "the sandbox is not created before the secret it has to be built with"
    );
    assert!(
        !world.ran("docker build"),
        "the image is not built before the missing secret is reported"
    );

    for host in crate::workflow::secret::GITHUB_HOSTS {
        world.secrets.borrow_mut().push(host.to_string());
    }
    let output = bench
        .build(&world, &request)
        .expect("the same add continues once the secret is registered");
    assert_eq!(output.worktrees.len(), 1);
    assert_eq!(
        world
            .invocations()
            .iter()
            .filter(|call| call.contains("sbx create"))
            .count(),
        1,
        "the sandbox that was already there is reused"
    );
}

#[test]
fn three_detached_worktrees_start_from_one_commit_of_the_named_branch() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", Some(3), Some("develop"));

    let output = bench.build(&world, &request).expect("build");
    assert_eq!(output.mode, CreationMode::Detached);
    assert_eq!(output.start_ref, "develop");
    assert_eq!(output.worktrees.len(), 3);
    for (index, worktree) in output.worktrees.iter().enumerate() {
        assert_eq!(worktree.path, format!("example-repo.tree-{index}"));
        assert_eq!(worktree.created_from, "refs/remotes/origin/develop");
        assert_eq!(worktree.head.as_deref(), Some(COMMIT));
        assert!(
                world.ran(&format!(
                    "worktree add --detach /home/agent/work/example-repo/example-repo.tree-{index} refs/remotes/origin/develop"
                )),
                "{:?}",
                world.invocations()
            );
    }
    // bare repositoryとworktreeは、1 treeでも3 treesでも分かれている。
    assert!(world.ran("git init --bare /home/agent/work/example-repo/.git"));
    assert!(world.ran("remote add origin https://github.com/Example-Org/Example-Repo.git"));
}

/// managed worktreeが持ち込んだ`mise.toml`のpath。
const DECLARED_MISE: &str = "/home/agent/work/example-repo/example-repo.tree-0/mise.toml";

#[test]
fn what_the_tools_answer_reaches_the_output() {
    let bench = bench();
    let world = World::new();
    world.carrying(DECLARED_MISE);

    let output = bench
        .build(&world, &request("Example-Org/Example-Repo", None, None))
        .expect("build");
    assert_eq!(output.notes.len(), 1, "{:?}", output.notes);
    assert_eq!(output.notes[0].items, vec![DECLARED_MISE.to_string()]);
}

#[test]
fn a_tool_the_sandbox_lacks_never_reaches_the_output() {
    let bench = bench();
    let world = World::new();
    world.without("mise");
    world.carrying(DECLARED_MISE);

    let output = bench
        .build(&world, &request("Example-Org/Example-Repo", None, None))
        .expect("build");
    assert!(output.notes.is_empty(), "{:?}", output.notes);
    assert!(
        !world.ran("mise.toml"),
        "the declared files are not even looked for: {:?}",
        world.invocations()
    );
}

#[test]
fn a_sandbox_without_gh_is_never_asked_to_configure_it() {
    let bench = bench();
    let world = World::new();
    world.without("gh");

    bench
        .build(&world, &request("Example-Org/Example-Repo", None, None))
        .expect("build");
    assert!(!world.ran("git_protocol"), "{:?}", world.invocations());
}

#[test]
fn the_long_steps_forward_their_progress_and_the_read_steps_are_captured() {
    let bench = bench();
    let world = World::new();
    bench
        .build(&world, &request("Example-Org/Example-Repo", None, None))
        .expect("build");

    for (needle, timeout) in [
        ("docker build", TimeoutClass::ImageBuild),
        ("docker image save", TimeoutClass::ImageBuild),
        ("git clone git@github.com", TimeoutClass::RepositoryTransfer),
        ("sbx template load", TimeoutClass::SandboxLifecycle),
        ("sbx create", TimeoutClass::SandboxLifecycle),
        ("fetch --prune origin", TimeoutClass::RepositoryTransfer),
    ] {
        assert_eq!(
            world.policy_of(needle),
            Some((OutputPolicy::Passthrough, timeout)),
            "{needle} shows its progress while it runs"
        );
    }

    for needle in [
        "sbx ls --json",
        "docker image inspect",
        "sbx secret ls",
        "sbx template ls --json",
    ] {
        assert_eq!(
            world.policy_of(needle).map(|(output, _)| output),
            Some(OutputPolicy::Capture),
            "{needle} is read rather than shown"
        );
    }
}

#[test]
fn the_declared_file_is_placed_once_and_left_alone_afterwards() {
    let bench = bench();
    let world = World::new();
    let request = request("Example-Org/Example-Repo", None, None);
    bench.build(&world, &request).expect("build");

    assert_eq!(
        world
            .digests
            .borrow()
            .get("/home/agent/.config/example/config.toml")
            .map(String::as_str),
        Some(sha256_hex(b"declared = true\n").as_str()),
        "the declared file reaches the destination it was declared for"
    );
    assert!(
        !world.present.borrow().contains("/tmp/sbxm-file-0"),
        "the staged copy does not survive the placement"
    );

    // 同じ内容の再配置は、Sandboxへ書き込まない。
    let world = World::new();
    world.digests.borrow_mut().insert(
        "/home/agent/.config/example/config.toml".to_string(),
        sha256_hex(b"declared = true\n"),
    );
    world
        .present
        .borrow_mut()
        .insert("/home/agent/.config/example/config.toml".to_string());
    let output = bench.build(&world, &request).expect("build");
    assert_eq!(output.files[0].placement, Placement::Unchanged);
    assert!(
        !world.ran("sbx cp"),
        "an identical destination is left alone"
    );
}
