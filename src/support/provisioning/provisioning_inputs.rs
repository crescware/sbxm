use std::fs;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

use crate::config::{FileDeclaration, GlobalConfig, HostFileSource};
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::hash::sha256_hex;
use crate::msg;
use crate::paths::{self, PRIVATE_DIR_MODE, PRIVATE_FILE_MODE, PathScope, ProjectPaths};
use crate::support::files;

use super::SnapshotFile;

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
        let dockerfile_path = paths.snapshot_dockerfile();
        let dockerfile_snapshot_written = dockerfile_sha256 == live_dockerfile_sha256;
        if dockerfile_snapshot_written {
            write_private_file(&dockerfile_path, &dockerfile_bytes)?;
        }

        let mut files = Vec::with_capacity(config.files.len());
        for (index, declaration) in config.files.iter().enumerate() {
            let (bytes, sha256) = files::read_source_bytes(declaration.source.as_path())?;
            let snapshot_path = paths.snapshot_file(index);
            write_private_file(&snapshot_path, &bytes)?;
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
            files.push(SnapshotFile {
                declaration: FileDeclaration {
                    source,
                    destination: declaration.destination.clone(),
                },
                sha256,
                original_source: paths::display(declaration.source.as_path()),
            });
        }

        Ok(ProvisioningInputs {
            dockerfile_path,
            dockerfile_sha256,
            dockerfile_snapshot_written,
            files,
        })
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

fn write_private_file(path: &Path, contents: &[u8]) -> Result<()> {
    let write = || -> std::io::Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(PRIVATE_FILE_MODE)
            .open(path)?;
        file.set_permissions(fs::Permissions::from_mode(PRIVATE_FILE_MODE))?;
        std::io::Write::write_all(&mut file, contents)?;
        file.sync_all()
    };
    write().map_err(|error| {
        Error::single(
            Diagnostic::new(
                ErrorId::AtomicWriteFailed,
                msg!("error-atomic-write-failed"),
            )
            .fact(Fact::path(&paths::display(path)))
            .fact(Fact::cause(&error.to_string())),
        )
    })
}

#[cfg(test)]
#[path = "provisioning_inputs_test.rs"]
mod provisioning_inputs_test;
