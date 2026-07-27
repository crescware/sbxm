//! Global configが宣言したfileのSandboxへの配置。
//!
//! 特定のAgentやtoolの設定形式を解釈せず、利用者が宣言したfileだけを、宣言された
//! 相対pathへ置く。file内容はstdout、stderr、log、metadataへ出さない。

use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::command::HostEnvironment;
use crate::config::FileDeclaration;
use crate::error::{Diagnostic, Error, ErrorId, Result};
use crate::hash::sha256_hex;
use crate::msg;
use crate::paths;

use super::sandbox;

/// Sandbox内の`agent` home。
const AGENT_HOME: &str = "/home/agent";

/// 1件あたりのsourceの上限。
const MAX_SOURCE_BYTES: u64 = 1024 * 1024;

/// 既存のdestinationと内容が異なる場合の扱い。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Conflict {
    /// `add`。構築の途中で利用者のfileを上書きしない。
    Refuse,
    /// `sync-files`。現在のglobal configを明示的な再配置要求として扱う。
    Overwrite,
}

/// 1件の配置結果。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Placement {
    /// 配置した。
    Placed,
    /// 既に同じ内容だったため何もしなかった。
    Unchanged,
}

impl Placement {
    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            Placement::Placed => "placed",
            Placement::Unchanged => "unchanged",
        }
    }
}

/// 1件の宣言に対する結果。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacedFile {
    pub source: PathBuf,
    /// `agent` homeからの相対path。
    pub destination: String,
    pub placement: Placement,
}

/// 宣言されたfileをSandboxへ配置する。
pub fn place_all(
    host: &dyn HostEnvironment,
    sandbox: &str,
    declarations: &[FileDeclaration],
    conflict: Conflict,
) -> Result<Vec<PlacedFile>> {
    let mut placed = Vec::with_capacity(declarations.len());
    for (index, declaration) in declarations.iter().enumerate() {
        placed.push(place(host, sandbox, index, declaration, conflict)?);
    }
    Ok(placed)
}

fn place(
    host: &dyn HostEnvironment,
    sandbox: &str,
    index: usize,
    declaration: &FileDeclaration,
    conflict: Conflict,
) -> Result<PlacedFile> {
    let source = declaration.source.as_path();
    let digest = read_source(source)?;
    let destination = destination_path(declaration.destination.as_path())?;
    let full = format!("{AGENT_HOME}/{destination}");
    // 宣言されたpath自体が`agent` home配下でも、Sandbox内のsymlinkが外を指し得る。
    require_no_symlink_in_sandbox(host, sandbox, source, &destination)?;

    if let Some(observed) = digest_in_sandbox(host, sandbox, &full)? {
        if observed == digest {
            return Ok(PlacedFile {
                source: source.to_path_buf(),
                destination,
                placement: Placement::Unchanged,
            });
        }
        if conflict == Conflict::Refuse {
            return Err(Error::single(
                Diagnostic::new(
                    ErrorId::DeclaredFileConflict,
                    msg!(
                        "error-declared-file-conflict",
                        source = paths::display(source),
                        destination = full
                    ),
                )
                .remediation(msg!("remediation-declared-file-conflict")),
            ));
        }
    }

    copy_into_sandbox(host, sandbox, index, source, &full)?;
    Ok(PlacedFile {
        source: source.to_path_buf(),
        destination,
        placement: Placement::Placed,
    })
}

/// sourceを検証し、そのSHA-256を返す。
fn read_source(source: &Path) -> Result<String> {
    let invalid = |detail: String| {
        Err(Error::new(
            ErrorId::DeclaredFileUnusable,
            msg!(
                "error-declared-file-unusable",
                source = paths::display(source),
                detail = detail
            ),
        ))
    };

    if !source.is_absolute() {
        return invalid("the source is not an absolute path".to_string());
    }
    let metadata = match fs::symlink_metadata(source) {
        Ok(metadata) => metadata,
        Err(error) => return invalid(format!("the source could not be read: {error}")),
    };
    if metadata.file_type().is_symlink() {
        return invalid("the source is a symbolic link".to_string());
    }
    if !metadata.is_file() {
        return invalid("the source is not a regular file".to_string());
    }
    if metadata.len() > MAX_SOURCE_BYTES {
        return invalid(format!(
            "the source is {} bytes, and sbxm places at most {MAX_SOURCE_BYTES}",
            metadata.len()
        ));
    }

    match fs::read(source) {
        // 内容は診断へ出さず、比較に使うdigestだけを持つ。
        Ok(contents) => Ok(sha256_hex(&contents)),
        Err(error) => invalid(format!("the source could not be read: {error}")),
    }
}

/// `agent` homeからの相対pathとして安全であることを確認する。
fn destination_path(destination: &Path) -> Result<String> {
    let invalid = |detail: &str| {
        Err(Error::new(
            ErrorId::DeclaredFileUnusable,
            msg!(
                "error-declared-file-unusable",
                source = paths::display(destination),
                detail = detail
            ),
        ))
    };

    if destination.is_absolute() {
        return invalid("the destination is an absolute path");
    }
    let mut parts = Vec::new();
    for component in destination.components() {
        match component {
            Component::Normal(part) => match part.to_str() {
                Some(part) => parts.push(part.to_string()),
                None => return invalid("the destination is not valid UTF-8"),
            },
            Component::CurDir => {}
            _ => return invalid("the destination leaves the agent home directory"),
        }
    }
    if parts.is_empty() {
        return invalid("the destination is empty");
    }
    Ok(parts.join("/"))
}

/// destinationが`agent` homeからsymlinkを経ずに届くことを確かめる。
///
/// 配置はroot権限で行うため、途中のcomponentがsymlinkであれば、read、chown、
/// 置き換えのいずれもhomeの外へ及ぶ。homeに近い側から1階層ずつ確認する。
fn require_no_symlink_in_sandbox(
    host: &dyn HostEnvironment,
    sandbox: &str,
    source: &Path,
    destination: &str,
) -> Result<()> {
    let mut current = AGENT_HOME.to_string();
    for part in destination.split('/') {
        current.push('/');
        current.push_str(part);
        if sandbox::exec(host, sandbox, &["test", "-h", &current])?.success() {
            return Err(Error::new(
                ErrorId::DeclaredFileUnusable,
                msg!(
                    "error-declared-file-unusable",
                    source = paths::display(source),
                    detail = format!("{current} is a symbolic link inside the sandbox")
                ),
            ));
        }
    }
    Ok(())
}

/// Sandbox内のdestinationのdigest。存在しない場合は`None`。
fn digest_in_sandbox(
    host: &dyn HostEnvironment,
    sandbox: &str,
    destination: &str,
) -> Result<Option<String>> {
    let exists = sandbox::exec(host, sandbox, &["test", "-e", destination])?;
    if !exists.success() {
        return Ok(None);
    }

    let outcome = sandbox::exec(host, sandbox, &["sha256sum", destination])?.require_success()?;
    let text = outcome.stdout_text();
    let digest = text.split_whitespace().next().unwrap_or_default();
    if digest.len() != 64 {
        return Err(Error::new(
            ErrorId::ExternalOutputUnparseable,
            msg!(
                "error-external-output-unparseable",
                program = "sha256sum",
                detail = format!("no digest was reported for {destination}")
            ),
        ));
    }
    Ok(Some(digest.to_string()))
}

/// 一時fileを経由して配置し、成功・失敗のどちらでも一時fileを削除する。
fn copy_into_sandbox(
    host: &dyn HostEnvironment,
    sandbox: &str,
    index: usize,
    source: &Path,
    destination: &str,
) -> Result<()> {
    let staged = format!("/tmp/sbxm-file-{index}");
    let pending = format!("{destination}.sbxm-new");
    let result = copy_steps(host, sandbox, source, &staged, destination, &pending);

    // 一時fileは成功・失敗のどちらでも残さない。
    let _ = sandbox::exec_as_root(host, sandbox, &["rm", "-f", &staged, &pending]);
    result
}

fn copy_steps(
    host: &dyn HostEnvironment,
    sandbox: &str,
    source: &Path,
    staged: &str,
    destination: &str,
    pending: &str,
) -> Result<()> {
    let spec = crate::command::CommandSpec::capture(
        "sbx",
        &[
            "cp",
            "--follow-link",
            &paths::display(source),
            &format!("{sandbox}:{staged}"),
        ],
    )
    .env(crate::command::EnvPolicy::InheritWithoutSshAgent)
    .timeout(crate::command::TimeoutClass::SandboxLifecycle);
    host.run(&spec)?.require_success()?;

    let parent = destination
        .rsplit_once('/')
        .map(|(parent, _)| parent.to_string())
        .unwrap_or_else(|| AGENT_HOME.to_string());
    sandbox::exec_as_root(
        host,
        sandbox,
        &[
            "install", "-d", "-o", "agent", "-g", "agent", "-m", "0700", &parent,
        ],
    )?
    .require_success()?;

    sandbox::exec_as_root(
        host,
        sandbox,
        &[
            "install", "-o", "agent", "-g", "agent", "-m", "0600", staged, pending,
        ],
    )?
    .require_success()?;

    // 置き換えはrenameで行い、読み手へ半端な内容を見せない。
    sandbox::exec_as_root(host, sandbox, &["mv", "-f", pending, destination])?.require_success()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::{CommandOutcome, CommandSpec};
    use crate::config::{HostFileSource, SandboxHomeRelativePath};
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::os::unix::process::ExitStatusExt;

    struct FakeSbx {
        /// Sandbox内のfileと、そのdigest。
        files: HashMap<String, String>,
        /// Sandbox内でsymlinkであるpath。
        symlinks: Vec<String>,
        calls: RefCell<Vec<Vec<String>>>,
    }

    impl FakeSbx {
        fn empty() -> FakeSbx {
            FakeSbx {
                files: HashMap::new(),
                symlinks: Vec::new(),
                calls: RefCell::new(Vec::new()),
            }
        }

        fn holding(destination: &str, contents: &[u8]) -> FakeSbx {
            let mut files = HashMap::new();
            files.insert(destination.to_string(), sha256_hex(contents));
            FakeSbx {
                files,
                symlinks: Vec::new(),
                calls: RefCell::new(Vec::new()),
            }
        }

        /// 指定したpathをsymlinkとして扱う。
        fn linking(mut self, path: &str) -> FakeSbx {
            self.symlinks.push(path.to_string());
            self
        }

        fn calls(&self) -> Vec<Vec<String>> {
            self.calls.borrow().clone()
        }

        fn ran(&self, needle: &str) -> bool {
            self.calls()
                .iter()
                .any(|args| args.iter().any(|arg| arg == needle))
        }
    }

    impl HostEnvironment for FakeSbx {
        fn command_exists(&self, _program: &str) -> bool {
            true
        }

        fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
            self.calls.borrow_mut().push(spec.args.clone());
            let mut code = 0;
            let mut stdout = String::new();

            if let Some(position) = spec.args.iter().position(|arg| arg == "--") {
                let inner = &spec.args[position + 1..];
                match inner.first().map(String::as_str) {
                    Some("test") => {
                        let target = inner.last().cloned().unwrap_or_default();
                        let present = match inner.get(1).map(String::as_str) {
                            Some("-h") => self.symlinks.contains(&target),
                            _ => self.files.contains_key(&target),
                        };
                        code = i32::from(!present);
                    }
                    Some("sha256sum") => {
                        let target = inner.last().cloned().unwrap_or_default();
                        match self.files.get(&target) {
                            Some(digest) => stdout = format!("{digest}  {target}\n"),
                            None => code = 1,
                        }
                    }
                    _ => {}
                }
            }

            Ok(CommandOutcome {
                program: spec.program.clone(),
                args: spec.args.clone(),
                working_dir: spec.working_dir.clone(),
                status: std::process::ExitStatus::from_raw(code << 8),
                stdout: stdout.into_bytes(),
                stderr: Vec::new(),
                stderr_lossy: false,
            })
        }
    }

    fn declaration(source: &Path, destination: &str) -> FileDeclaration {
        FileDeclaration {
            source: HostFileSource::new(&paths::display(source)).expect("valid source"),
            destination: SandboxHomeRelativePath::new(destination).expect("valid destination"),
        }
    }

    fn source_file(dir: &Path, contents: &[u8]) -> PathBuf {
        let path = dir.join("declared.toml");
        fs::write(&path, contents).expect("write the source");
        path
    }

    #[test]
    fn a_declared_file_is_staged_installed_and_moved_into_place() {
        let dir = tempfile::tempdir().unwrap();
        let source = source_file(dir.path(), b"declared = true\n");
        let host = FakeSbx::empty();

        let placed = place_all(
            &host,
            "sbxm-example",
            &[declaration(&source, ".config/example/config.toml")],
            Conflict::Refuse,
        )
        .expect("place");

        assert_eq!(
            placed,
            vec![PlacedFile {
                source: source.clone(),
                destination: ".config/example/config.toml".to_string(),
                placement: Placement::Placed,
            }]
        );

        let calls = host.calls();
        assert!(
            calls.iter().any(|args| args
                == &vec![
                    "cp".to_string(),
                    "--follow-link".to_string(),
                    paths::display(&source),
                    "sbxm-example:/tmp/sbxm-file-0".to_string()
                ]),
            "{calls:?}"
        );
        assert!(
            calls
                .iter()
                .any(|args| args.contains(&"install".to_string())
                    && args.contains(&"0700".to_string())
                    && args.contains(&"/home/agent/.config/example".to_string())),
            "the parent directory is private: {calls:?}"
        );
        assert!(
            calls
                .iter()
                .any(|args| args.contains(&"install".to_string())
                    && args.contains(&"0600".to_string())
                    && args.contains(&"agent".to_string())),
            "the file belongs to the agent and is private: {calls:?}"
        );
        assert!(
            calls.iter().any(|args| args.contains(&"mv".to_string())
                && args.contains(&"/home/agent/.config/example/config.toml".to_string())),
            "the destination is replaced by a rename: {calls:?}"
        );
        assert!(
            host.ran("/tmp/sbxm-file-0"),
            "the staged copy is removed afterwards: {calls:?}"
        );
    }

    #[test]
    fn a_destination_that_already_holds_the_same_content_is_left_alone() {
        let dir = tempfile::tempdir().unwrap();
        let contents = b"declared = true\n";
        let source = source_file(dir.path(), contents);
        let host = FakeSbx::holding("/home/agent/.config/example/config.toml", contents);

        let placed = place_all(
            &host,
            "sbxm-example",
            &[declaration(&source, ".config/example/config.toml")],
            Conflict::Refuse,
        )
        .expect("place");

        assert_eq!(placed[0].placement, Placement::Unchanged);
        assert!(
            !host.ran("cp"),
            "nothing is copied when the content already matches"
        );
    }

    #[test]
    fn add_refuses_to_overwrite_a_different_file_while_sync_files_replaces_it() {
        let dir = tempfile::tempdir().unwrap();
        let source = source_file(dir.path(), b"new contents\n");
        let declarations = [declaration(&source, ".config/example/config.toml")];

        let host = FakeSbx::holding("/home/agent/.config/example/config.toml", b"older\n");
        let error = place_all(&host, "sbxm-example", &declarations, Conflict::Refuse)
            .expect_err("a build never overwrites what is already there");
        assert_eq!(error.first_id(), Some(ErrorId::DeclaredFileConflict));
        assert!(!host.ran("cp"));

        let host = FakeSbx::holding("/home/agent/.config/example/config.toml", b"older\n");
        let placed = place_all(&host, "sbxm-example", &declarations, Conflict::Overwrite)
            .expect("an explicit re-placement replaces it");
        assert_eq!(placed[0].placement, Placement::Placed);
        assert!(host.ran("mv"));
    }

    #[test]
    fn a_source_that_cannot_be_placed_safely_stops_before_anything_is_copied() {
        let dir = tempfile::tempdir().unwrap();
        let real = source_file(dir.path(), b"declared\n");

        let link = dir.path().join("link.toml");
        std::os::unix::fs::symlink(&real, &link).unwrap();
        let directory = dir.path().join("a-directory");
        fs::create_dir(&directory).unwrap();
        let large = dir.path().join("large.bin");
        fs::write(&large, vec![0_u8; (MAX_SOURCE_BYTES + 1) as usize]).unwrap();
        let absent = dir.path().join("absent.toml");

        for source in [link, directory, large, absent] {
            let host = FakeSbx::empty();
            let error = place_all(
                &host,
                "sbxm-example",
                &[declaration(&source, ".config/example/config.toml")],
                Conflict::Refuse,
            )
            .expect_err("{source:?} must be refused");
            assert_eq!(
                error.first_id(),
                Some(ErrorId::DeclaredFileUnusable),
                "source {source:?} produced the wrong error"
            );
            assert!(host.calls().is_empty(), "nothing is asked of the sandbox");
        }
    }

    #[test]
    fn a_destination_reached_through_a_symbolic_link_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let source = source_file(dir.path(), b"declared = true\n");
        let declarations = [declaration(&source, ".config/example/config.toml")];

        // 途中のdirectoryも、destination自身も、homeの外を指し得る。
        for link in [
            "/home/agent/.config",
            "/home/agent/.config/example",
            "/home/agent/.config/example/config.toml",
        ] {
            for conflict in [Conflict::Refuse, Conflict::Overwrite] {
                let host = FakeSbx::empty().linking(link);
                let error = place_all(&host, "sbxm-example", &declarations, conflict)
                    .expect_err("a path that leaves the agent home is not written to");
                assert_eq!(
                    error.first_id(),
                    Some(ErrorId::DeclaredFileUnusable),
                    "{link} produced the wrong error"
                );
                assert!(
                    !host.ran("cp") && !host.ran("install") && !host.ran("mv"),
                    "nothing is copied or installed through {link}: {:?}",
                    host.calls()
                );
                assert!(
                    !host.ran("sha256sum"),
                    "the destination is not even read through {link}: {:?}",
                    host.calls()
                );
            }
        }
    }

    #[test]
    fn every_step_of_the_destination_is_checked_for_a_symbolic_link() {
        let dir = tempfile::tempdir().unwrap();
        let source = source_file(dir.path(), b"declared = true\n");
        let host = FakeSbx::empty();

        place_all(
            &host,
            "sbxm-example",
            &[declaration(&source, ".config/example/config.toml")],
            Conflict::Refuse,
        )
        .expect("place");

        for step in [
            "/home/agent/.config",
            "/home/agent/.config/example",
            "/home/agent/.config/example/config.toml",
        ] {
            assert!(
                host.calls().iter().any(
                    |args| args.contains(&"-h".to_string()) && args.contains(&step.to_string())
                ),
                "{step} is checked: {:?}",
                host.calls()
            );
        }
    }

    #[test]
    fn the_content_of_a_declared_file_never_reaches_a_diagnostic() {
        let dir = tempfile::tempdir().unwrap();
        let secret = "a-value-that-must-not-be-shown";
        let source = source_file(dir.path(), secret.as_bytes());
        let host = FakeSbx::holding("/home/agent/.config/example/config.toml", b"older\n");

        let error = place_all(
            &host,
            "sbxm-example",
            &[declaration(&source, ".config/example/config.toml")],
            Conflict::Refuse,
        )
        .expect_err("the conflict is reported");

        let rendered = format!("{error:?}");
        assert!(
            !rendered.contains(secret),
            "the diagnostic must name the paths only: {rendered}"
        );
        for args in host.calls() {
            assert!(
                !args.iter().any(|arg| arg.contains(secret)),
                "the content never reaches an argument: {args:?}"
            );
        }
    }
}
