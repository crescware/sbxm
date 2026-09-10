use std::fs::{self, File};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::config::{FileDeclaration, GlobalConfig, HostFileSource, SandboxHomeRelativePath};
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::hash::sha256_hex;
use crate::msg;
use crate::paths::{self, PRIVATE_DIR_MODE, PRIVATE_FILE_MODE, PathScope, ProjectPaths};
use crate::support::files;

use super::SnapshotFile;
use crate::metadata::InitialProvisioningIntent;

/// 最初のmutationより前に固定する、初回構築の入力一式。
///
/// Dockerfileと全宣言fileを1回だけ読み、project配下のprivateなsnapshotへ複製する。
/// 以降の`provision`はこのsnapshotだけを読み、生きているhost pathを二度と読まない。
/// generationとintentは、この同じbyte列から作る。
#[derive(Debug)]
pub(crate) struct ProvisioningInputs {
    pub dockerfile_path: PathBuf,
    pub dockerfile_sha256: String,
    /// `dockerfile_path`が、実際にこの実行のためのsnapshotとして書かれているか。
    ///
    /// repairが固定済みのtarget generationへ向かう場合、現在のDockerfileが別世代を
    /// 表していることがある。その場合は現在の内容をこのtargetのsnapshotとして書かない。
    /// 対象世代のimageは既存の検証済み成果物としてだけ再利用され、buildへは進まない。
    dockerfile_snapshot_written: bool,
    pub files: Vec<SnapshotFile>,
}

impl ProvisioningInputs {
    /// hostのDockerfileと宣言fileを読み、snapshotを作る。
    ///
    /// `target_generation`を指定すると、そのgenerationへ向けてsnapshotを固定する。
    /// 現在のDockerfileが別のgenerationを表す場合、Dockerfileのsnapshotは作らない
    /// （そのgenerationのimageは既存成果物の再利用でしか進めない）。`None`は最初の
    /// 構築であり、現在のDockerfileがそのままtargetになる。
    pub(crate) fn capture(
        paths: &ProjectPaths,
        config: &GlobalConfig,
        target_generation: Option<&str>,
    ) -> Result<ProvisioningInputs> {
        let snapshot_dir = paths.snapshot_dir();
        paths::ensure_private_dir(&snapshot_dir, PRIVATE_DIR_MODE, PathScope::ProjectPath)?;

        let dockerfile_bytes = read_dockerfile(paths)?;
        let live_dockerfile_sha256 = sha256_hex(&dockerfile_bytes);
        let dockerfile_sha256 =
            target_generation.map_or_else(|| live_dockerfile_sha256.clone(), str::to_string);
        let dockerfile_path = paths.snapshot_blob(&dockerfile_sha256);
        let dockerfile_snapshot_written = dockerfile_sha256 == live_dockerfile_sha256;
        if dockerfile_snapshot_written {
            write_blob(paths, &dockerfile_sha256, &dockerfile_bytes)?;
        }

        let files = Self::capture_files(paths, config)?;

        Ok(ProvisioningInputs {
            dockerfile_path,
            dockerfile_sha256,
            dockerfile_snapshot_written,
            files,
        })
    }

    /// 保存済みintentの入力を、内容digestで固定したblobから読み直す。
    ///
    /// 0.0.11が残した位置固定snapshotと、記録された元sourceは移行元としてだけ読む。
    /// digestが一致したbyte列は内容別blobへ取り込み、現在のglobal configとの一致は
    /// 再開条件にしない。
    pub(crate) fn resume(
        paths: &ProjectPaths,
        intent: &InitialProvisioningIntent,
        require_dockerfile: bool,
    ) -> Result<ProvisioningInputs> {
        let dockerfile_path = if require_dockerfile {
            resolve_recorded(
                paths,
                &intent.target_dockerfile_sha256,
                &paths.snapshot_dockerfile(),
                &paths.dockerfile(),
            )?
        } else {
            import_recorded_if_available(
                paths,
                &intent.target_dockerfile_sha256,
                &paths.snapshot_dockerfile(),
                &paths.dockerfile(),
            )?
        };
        let mut files = Vec::with_capacity(intent.files.len());
        for (index, recorded) in intent.files.iter().enumerate() {
            let original = std::path::PathBuf::from(&recorded.source);
            let snapshot_path = resolve_recorded(
                paths,
                &recorded.sha256,
                &paths.snapshot_file(index),
                &original,
            )?;
            let source =
                HostFileSource::new(&paths::display(&snapshot_path)).map_err(invalid_recorded)?;
            let destination =
                SandboxHomeRelativePath::new(&recorded.destination).map_err(invalid_recorded)?;
            files.push(SnapshotFile {
                declaration: FileDeclaration {
                    source,
                    destination,
                },
                sha256: recorded.sha256.clone(),
                original_source: recorded.source.clone(),
            });
        }
        let dockerfile_snapshot_written = require_dockerfile || dockerfile_path.exists();
        Ok(ProvisioningInputs {
            dockerfile_path,
            dockerfile_sha256: intent.target_dockerfile_sha256.clone(),
            dockerfile_snapshot_written,
            files,
        })
    }

    /// 宣言fileだけを内容別blobへ固定する。明示applyはDockerfileを必要としない。
    pub(crate) fn capture_files(
        paths: &ProjectPaths,
        config: &GlobalConfig,
    ) -> Result<Vec<SnapshotFile>> {
        let mut captured = Vec::with_capacity(config.files.len());
        for declaration in &config.files {
            let (bytes, sha256) = files::read_source_bytes(declaration.source.as_path())?;
            let snapshot_path = paths.snapshot_blob(&sha256);
            write_blob(paths, &sha256, &bytes)?;
            let source =
                HostFileSource::new(&paths::display(&snapshot_path)).map_err(|reason| {
                    Error::single(
                        Diagnostic::new(
                            ErrorId::DeclaredFileUnusable,
                            msg!("error-declared-file-unusable"),
                        )
                        .fact(Fact::reason(reason)),
                    )
                })?;
            captured.push(SnapshotFile {
                declaration: FileDeclaration {
                    source,
                    destination: declaration.destination.clone(),
                },
                sha256,
                original_source: paths::display(declaration.source.as_path()),
            });
        }
        Ok(captured)
    }

    /// 配置に使うための、snapshot宛の宣言だけを並べる。
    pub(crate) fn file_declarations(&self) -> Vec<FileDeclaration> {
        self.files
            .iter()
            .map(|file| file.declaration.clone())
            .collect()
    }

    /// snapshotが、作った時点のbyte列のままであることを確かめる。
    ///
    /// snapshotはこの実行の間だけ生きるprivate fileであり、他に書き手はいないはずだが、
    /// 実際に使う直前でもう一度確かめてから、build・copyへ渡す。入れ替わっていれば
    /// 拒否し、intentは残す。
    pub(crate) fn verify_unchanged(&self) -> Result<()> {
        if self.dockerfile_snapshot_written {
            verify_snapshot(&self.dockerfile_path, &self.dockerfile_sha256)?;
        }
        for file in &self.files {
            verify_snapshot(file.declaration.source.as_path(), &file.sha256)?;
        }
        Ok(())
    }
}

fn read_dockerfile(paths: &ProjectPaths) -> Result<Vec<u8>> {
    let path = paths.dockerfile();
    if !paths::regular_file_exists(&path, PathScope::ProjectPath)? {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::ProjectPathUnreadable,
                msg!("error-project-path-unreadable"),
            )
            .fact(Fact::path(&paths::display(&path)))
            .fact(Fact::reason(msg!("cause-dockerfile-absent"))),
        ));
    }
    fs::read(&path)
        .map_err(|error| PathScope::ProjectPath.unreadable_error(&path, &error.to_string()))
}

fn verify_snapshot(path: &Path, expected_sha256: &str) -> Result<()> {
    let bytes = fs::read(path).map_err(|error| {
        Error::single(
            Diagnostic::new(
                ErrorId::InitialProvisioningSnapshotChanged,
                msg!("error-initial-provisioning-snapshot-changed"),
            )
            .fact(Fact::path(&paths::display(path)))
            .fact(Fact::cause(&error.to_string())),
        )
    })?;
    let observed = sha256_hex(&bytes);
    if observed != expected_sha256 {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::InitialProvisioningSnapshotChanged,
                msg!("error-initial-provisioning-snapshot-changed"),
            )
            .fact(Fact::path(&paths::display(path)))
            .fact(Fact::reason(msg!(
                "cause-initial-provisioning-snapshot-changed"
            ))),
        ));
    }
    Ok(())
}

fn resolve_recorded(
    paths: &ProjectPaths,
    expected: &str,
    legacy: &Path,
    original: &Path,
) -> Result<std::path::PathBuf> {
    let blob = paths.snapshot_blob(expected);
    if blob.exists() {
        verify_snapshot(&blob, expected)?;
        return Ok(blob);
    }
    for candidate in [legacy, original] {
        let Ok(bytes) = fs::read(candidate) else {
            continue;
        };
        if sha256_hex(&bytes) == expected {
            write_blob(paths, expected, &bytes)?;
            return Ok(blob);
        }
    }
    verify_snapshot(&blob, expected)?;
    Ok(blob)
}

fn import_recorded_if_available(
    paths: &ProjectPaths,
    expected: &str,
    legacy: &Path,
    original: &Path,
) -> Result<std::path::PathBuf> {
    let blob = paths.snapshot_blob(expected);
    if blob.exists() {
        verify_snapshot(&blob, expected)?;
        return Ok(blob);
    }
    for candidate in [legacy, original] {
        let Ok(bytes) = fs::read(candidate) else {
            continue;
        };
        if sha256_hex(&bytes) == expected {
            write_blob(paths, expected, &bytes)?;
            break;
        }
    }
    Ok(blob)
}

fn write_blob(paths: &ProjectPaths, digest: &str, contents: &[u8]) -> Result<()> {
    let directory = paths.snapshot_dir().join("sha256");
    paths::ensure_private_dir(&directory, PRIVATE_DIR_MODE, PathScope::ProjectPath)?;
    let target = paths.snapshot_blob(digest);
    if target.exists() {
        return verify_snapshot(&target, digest);
    }
    let write = || -> std::io::Result<()> {
        let mut temporary = tempfile::Builder::new()
            .prefix(".snapshot-")
            .tempfile_in(&directory)?;
        temporary
            .as_file()
            .set_permissions(fs::Permissions::from_mode(PRIVATE_FILE_MODE))?;
        temporary.write_all(contents)?;
        temporary.as_file().sync_all()?;
        match temporary.persist_noclobber(&target) {
            Ok(_) => {}
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                let observed = fs::read(&target)?;
                if sha256_hex(&observed) != digest {
                    return Err(std::io::Error::other("existing snapshot digest differs"));
                }
            }
            Err(error) => return Err(error.error),
        }
        if let Ok(parent) = File::open(&directory) {
            let _ = parent.sync_all();
        }
        Ok(())
    };
    write().map_err(|error| paths::atomic_write_failed(&target, &error.to_string()))
}

fn invalid_recorded(reason: crate::diagnostics::Msg) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::InitialProvisioningSnapshotChanged,
            msg!("error-initial-provisioning-snapshot-changed"),
        )
        .fact(Fact::reason(reason)),
    )
}

#[cfg(test)]
#[path = "provisioning_inputs_test.rs"]
mod provisioning_inputs_test;
