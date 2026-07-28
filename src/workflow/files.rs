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
#[path = "files_test.rs"]
mod files_test;
