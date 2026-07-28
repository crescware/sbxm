//! `sbxm prepare`。
//!
//! 登録済み案件のSandboxを作り、作業できる状態にする。案件の登録とhost cloneは
//! `add`が終えており、ここから先は目標構成をmetadataから読む。
//!
//! 中断した案件へ同じcommandを再実行すると、成功済みの工程をinspectしてskipし、
//! 最初の未完了工程から続ける。

use std::path::Path;

use crate::command::HostEnvironment;
use crate::config::GlobalConfig;
use crate::error::{Diagnostic, Error, ErrorId, Msg, Result};
use crate::metadata::{self, CreationMode, ProjectMetadata};
use crate::msg;
use crate::paths::{self, LOCK_TIMEOUT, PRIVATE_FILE_MODE, PathScope, ProjectPaths};
use crate::project::{ProjectId, SandboxLayout, SandboxName};

use super::files::PlacedFile;
use super::{daemon, files, identity, image, repository, sandbox, secret, template};

/// `mise`の設定を持つと判断するfile。
const MISE_FILES: [&str; 3] = ["mise.toml", ".mise.toml", ".tool-versions"];

/// 出力のworktree 1行。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRow {
    pub path: String,
    pub created_from: String,
    /// 観測できたHEAD。停止中のSandboxでは読めないため`None`になる。
    pub head: Option<String>,
    pub mode: CreationMode,
}

/// `prepare`の結果。
#[derive(Debug, Clone)]
pub struct PrepareOutput {
    pub project: String,
    pub sandbox: String,
    pub mode: CreationMode,
    pub start_ref: String,
    pub sandbox_state: crate::compatibility::SandboxState,
    pub worktrees: Vec<WorktreeRow>,
    pub files: Vec<PlacedFile>,
    /// `mise`の設定を持つmanaged worktree。sbxmは自動実行せず案内だけを行う。
    pub mise_candidates: Vec<String>,
    /// 既に構築済みで、この実行が何も変更しなかったか。
    pub already_built: bool,
    pub warnings: Vec<Msg>,
}

/// 登録済み案件のSandboxを構築する。
pub fn run(
    config: &GlobalConfig,
    project: &ProjectId,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<PrepareOutput> {
    let canonical = project.canonical();
    let paths = ProjectPaths::derive(&config.base_path, &canonical);
    let name = SandboxName::derive(&canonical);

    // 対象が登録されていない案件にlock fileを作らない。
    if metadata::load(&paths)?.is_none() {
        return Err(not_registered(project));
    }
    let _lock = paths::acquire_exclusive_lock(
        &paths.lock_file(),
        LOCK_TIMEOUT,
        PRIVATE_FILE_MODE,
        PathScope::ProjectPath,
    )?;

    // lockを取る前に読んだmetadataは古くなり得る。判定はlock後の内容だけで行う。
    let mut project_metadata = metadata::load(&paths)?.ok_or_else(|| not_registered(project))?;
    require_no_rebuild(&project_metadata)?;

    let layout = SandboxLayout::new(&canonical);
    let mut warnings = Vec::new();

    if let Some(output) = already_built(
        host,
        &paths,
        &name,
        &project_metadata,
        &layout,
        workspace_root,
    )? {
        return Ok(output);
    }

    let current = super::add::current_dockerfile_hash(&paths)?;
    let generation = adopt_generation(
        host,
        &paths,
        &mut project_metadata,
        &name,
        &current,
        &mut warnings,
    )?;

    let built = image::ensure(
        host,
        &name,
        &project_metadata.canonical_id,
        &paths.dockerfile(),
        &generation,
    )?;
    warnings.extend(built.warnings.clone());
    let archive = image::ensure_archive(host, &paths, &built, &generation)?;
    let loaded = template::ensure(host, &archive, &built)?;

    let ready = sandbox::ensure(host, &name, &loaded, workspace_root)?;
    // hostのSSH Agentが届かないことを、daemonの起動条件から推定せず中から確かめる。
    sandbox::require_credentials_isolated(host, &ready.name)?;

    let files = files::place_all(host, &ready.name, &config.files, files::Conflict::Refuse)?;
    identity::ensure(host, &ready.name, &config.git)?;
    secret::require_github(host, &ready.name)?;

    repository::ensure_bare_clone(host, &ready.name, project, &layout)?;
    let branch =
        repository::resolve_start_ref(host, &ready.name, &layout, &paths, &mut project_metadata)?;
    let managed = repository::ensure_worktrees(
        host,
        &ready.name,
        &layout,
        &paths,
        &mut project_metadata,
        &branch,
    )?;

    let worktrees = observed_worktrees(host, &ready.name, &layout, &project_metadata)?;
    let mise_candidates = mise_candidates(host, &ready.name, &layout, managed.len())?;

    Ok(PrepareOutput {
        project: project_metadata.display_id(),
        sandbox: ready.name,
        mode: project_metadata.provisioning.mode,
        start_ref: branch,
        sandbox_state: ready.state,
        worktrees,
        files,
        mise_candidates,
        already_built: false,
        warnings,
    })
}

/// 目標構成をすべて満たしたSandboxが既にあるか。
///
/// ある場合は副作用なしのno-op成功とする。判定はmetadataの完全性だけで済ませず、
/// Sandbox identityまで確認する。
fn already_built(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    name: &SandboxName,
    metadata: &ProjectMetadata,
    layout: &SandboxLayout,
    workspace_root: &Path,
) -> Result<Option<PrepareOutput>> {
    let _ = paths;
    let provisioning = &metadata.provisioning;
    if provisioning.start_ref.is_none()
        || metadata.managed_worktrees.len() != provisioning.requested_worktrees as usize
    {
        return Ok(None);
    }

    let sandboxes = daemon::list(host)?;
    let Some(entry) = sandboxes
        .into_iter()
        .find(|entry| entry.name == name.as_str())
    else {
        return Ok(None);
    };

    // Templateは、metadataが正本とする世代から導出する。
    let templates = image::template_names(name, metadata);
    sandbox::verify_identity(&entry, name, &templates, workspace_root)?;

    let worktrees = observed_worktrees(host, &entry.name, layout, metadata)?;
    Ok(Some(PrepareOutput {
        project: metadata.display_id(),
        sandbox: entry.name,
        mode: provisioning.mode,
        start_ref: provisioning.start_ref.clone().unwrap_or_default(),
        sandbox_state: entry.state,
        worktrees,
        files: Vec::new(),
        mise_candidates: Vec::new(),
        already_built: true,
        warnings: Vec::new(),
    }))
}

/// 初回構築を完成させる世代を決める。
///
/// image buildの前にDockerfileが変わった場合は、現在のDockerfileを目標とする。
/// 既にimageがある場合は保存済み世代で完成させ、現在の内容は`rebuild`へ案内する。
fn adopt_generation(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    metadata: &mut ProjectMetadata,
    name: &SandboxName,
    current: &str,
    warnings: &mut Vec<Msg>,
) -> Result<String> {
    let stored = metadata.provisioning.dockerfile_sha256.clone();
    if current == stored {
        return Ok(stored);
    }

    if image::generation_is_built(host, name, &metadata.canonical_id, &stored)? {
        // 初回構築の途中へ別世代を混在させない。
        warnings.push(msg!(
            "warning-dockerfile-changed-during-build",
            project = metadata.display_id(),
            command = format!("sbxm rebuild {}", metadata.display_id())
        ));
        return Ok(stored);
    }

    metadata.provisioning.dockerfile_sha256 = current.to_string();
    metadata::update(paths, metadata)?;
    Ok(current.to_string())
}

/// metadataが宣言するmanaged worktreeの現在の状態。
fn observed_worktrees(
    host: &dyn HostEnvironment,
    sandbox: &str,
    layout: &SandboxLayout,
    metadata: &ProjectMetadata,
) -> Result<Vec<WorktreeRow>> {
    let mode = metadata.provisioning.mode;
    let mut rows = Vec::with_capacity(metadata.managed_worktrees.len());
    for worktree in &metadata.managed_worktrees {
        let path = format!("{}/{}", layout.bare_root(), worktree.path);
        // 停止中のSandboxではHEADを読めない。観測できない値を推測で埋めない。
        let outcome = sandbox::exec(host, sandbox, &["git", "-C", &path, "rev-parse", "HEAD"])?;
        let head = outcome
            .success()
            .then(|| outcome.stdout_text().trim().to_string())
            .filter(|head| !head.is_empty());
        rows.push(WorktreeRow {
            path: worktree.path.clone(),
            created_from: worktree.created_from.clone(),
            head,
            mode,
        });
    }
    Ok(rows)
}

/// `mise`の設定を持つmanaged worktree。
fn mise_candidates(
    host: &dyn HostEnvironment,
    sandbox: &str,
    layout: &SandboxLayout,
    count: usize,
) -> Result<Vec<String>> {
    let mut candidates = Vec::new();
    for index in 0..count as u32 {
        let path = layout.worktree(index);
        for name in MISE_FILES {
            let target = format!("{path}/{name}");
            if sandbox::exec(host, sandbox, &["test", "-f", &target])?.success() {
                candidates.push(target);
            }
        }
    }
    Ok(candidates)
}

/// 登録されていない案件は構築できない。
fn not_registered(project: &ProjectId) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::ProjectNotManaged,
            msg!("error-project-not-managed", project = project),
        )
        .remediation(msg!(
            "remediation-project-not-managed",
            command = format!("sbxm add {project}")
        )),
    )
}

/// 世代の切替中は構築を進めず、`rebuild`の完了を案内する。
fn require_no_rebuild(metadata: &ProjectMetadata) -> Result<()> {
    if metadata.rebuild.is_none() {
        return Ok(());
    }
    Err(Error::single(
        Diagnostic::new(
            ErrorId::RebuildIntentPending,
            msg!(
                "error-rebuild-intent-pending",
                project = metadata.display_id()
            ),
        )
        .remediation(msg!(
            "remediation-run-rebuild",
            command = format!("sbxm rebuild {}", metadata.display_id())
        )),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandOutcome, OutputPolicy, TimeoutClass};
    use crate::compatibility::SandboxState;
    use crate::config::GitIdentity;
    use crate::error::Result;
    use crate::hash::sha256_hex;
    use crate::i18n::Locale;
    use crate::paths::{AbsoluteBasePath, PRIVATE_DIR_MODE};
    use crate::workflow::add::AddRequest;
    use crate::workflow::add::tests::{COMMIT, request};
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

    /// docker、`sbx`、gitの応答を状態として持ち、`add`の全工程を通せるhost。
    ///
    /// 各工程の副作用は、その工程が成功したときにだけ起こす。中断した実行の続きを
    /// 同じ`add`が進められるかどうかは、この性質の上で判定できる。
    struct World {
        /// tag -> buildが宣言したlabel。
        images: RefCell<BTreeMap<String, Vec<(String, String)>>>,
        /// Template名 -> 対応するimage ID。
        templates: RefCell<BTreeMap<String, String>>,
        sandboxes: RefCell<Vec<SandboxRow>>,
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
    }

    const IMAGE_ID: &str =
        "sha256:3333333333333333333333333333333333333333333333333333333333333333";

    impl World {
        fn new() -> World {
            World {
                images: RefCell::new(BTreeMap::new()),
                templates: RefCell::new(BTreeMap::new()),
                sandboxes: RefCell::new(Vec::new()),
                secrets: RefCell::new(vec!["github".to_string()]),
                present: RefCell::new(BTreeSet::new()),
                digests: RefCell::new(BTreeMap::new()),
                settings: RefCell::new(BTreeMap::new()),
                repository: RefCell::new(BTreeMap::new()),
                worktrees: RefCell::new(BTreeMap::new()),
                default_branch: "main".to_string(),
                fail: RefCell::new(None),
                calls: RefCell::new(Vec::new()),
            }
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
            use std::os::unix::process::ExitStatusExt;
            CommandOutcome {
                program: spec.program.clone(),
                args: spec.args.clone(),
                working_dir: spec.working_dir.clone(),
                status: std::process::ExitStatus::from_raw(code << 8),
                stdout: stdout.as_bytes().to_vec(),
                stderr: Vec::new(),
                stderr_lossy: false,
            }
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
                            format!(
                                r#"[{{"Id":"{IMAGE_ID}","Config":{{"Labels":{{{rendered}}}}}}}]"#
                            ),
                        )
                    }
                    None => (1, String::new()),
                },
                ["image", "save", name, "--output", output] => {
                    // 実物と同じく、archiveはimage configをlabelごと持つ。
                    let labels = self.images.borrow().get(*name).cloned().unwrap_or_default();
                    let rendered = labels
                        .iter()
                        .map(|(key, value)| format!("\"{key}\":\"{value}\""))
                        .collect::<Vec<_>>()
                        .join(",");
                    let config = format!(r#"{{"config":{{"Labels":{{{rendered}}}}}}}"#);
                    let hex = IMAGE_ID.strip_prefix("sha256:").expect("a digest");
                    fs::write(
                        output,
                        crate::archive::tar_bytes(&[
                            (&format!("blobs/sha256/{hex}"), config.as_bytes()),
                            (
                                "manifest.json",
                                crate::archive::manifest_json(name, IMAGE_ID).as_bytes(),
                            ),
                        ]),
                    )
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
                    self.sandboxes.borrow_mut().push(SandboxRow {
                        name: name.to_string(),
                        workspace: workspace.to_string(),
                        template: template.to_string(),
                    });
                    (0, String::new())
                }
                ["secret", "ls", name] => {
                    let secrets = self.secrets.borrow();
                    if secrets.is_empty() {
                        return (0, format!("No secrets found for scope \"{name}\".\n"));
                    }
                    let mut table = String::from("SCOPE   TYPE      NAME     SECRET\n");
                    for secret in secrets.iter() {
                        table.push_str(&format!("{name}   service   {secret}   (stored)\n"));
                    }
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
            let missing = (1, String::new());
            let ok = (0, String::new());

            match inner {
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
                ["git", "config", "--global", "--get", key] => {
                    match self.settings.borrow().get(*key) {
                        Some(value) => (0, format!("{value}\n")),
                        None => missing,
                    }
                }
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
                ["git", "clone", "--bare", url, git_dir] => {
                    self.present.borrow_mut().insert(git_dir.to_string());
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
                destination: crate::config::SandboxHomeRelativePath::new(
                    ".config/example/config.toml",
                )
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

    /// `add`が外部工程を呼ぶ順に並べた、失敗させる工程とその診断。
    const STEPS: [(&str, ErrorId); 11] = [
        ("git clone git@github.com", ErrorId::ExternalCommandFailed),
        ("docker build", ErrorId::ExternalCommandFailed),
        ("docker image save", ErrorId::ExternalCommandFailed),
        ("sbx template load", ErrorId::ExternalCommandFailed),
        ("sbx create", ErrorId::ExternalCommandFailed),
        ("sbx cp --follow-link", ErrorId::ExternalCommandFailed),
        ("config --global user.name", ErrorId::ExternalCommandFailed),
        ("sbx secret ls", ErrorId::ExternalCommandFailed),
        ("git clone --bare", ErrorId::ExternalCommandFailed),
        ("check-ref-format", ErrorId::InvalidBranchName),
        ("worktree add", ErrorId::ExternalCommandFailed),
    ];

    #[test]
    fn an_interruption_at_any_step_is_continued_by_the_same_add() {
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
        assert_eq!(stored.managed_worktrees.len(), 1);

        // 成功済みの成果物は作り直さない。
        for done in [
            "git clone git@github.com",
            "docker build",
            "sbx template load",
            "sbx create",
            "git clone --bare",
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
            !world.ran("git clone --bare"),
            "the sandbox repository is not cloned without the secret"
        );

        world.secrets.borrow_mut().push("github".to_string());
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
        assert!(world.ran("git clone --bare https://github.com/Example-Org/Example-Repo.git /home/agent/work/example-repo/.git"));
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
            ("git clone --bare", TimeoutClass::RepositoryTransfer),
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
}
