use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::boundary::host::HostEnvironment;
use crate::config::{HostFileSource, SandboxHomeRelativePath};
use crate::design::{Fact, Remediation};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::{self, InitialProvisioningFile, ProjectMetadata};
use crate::msg;
use crate::paths::{self, PRIVATE_FILE_MODE, PathScope, ProjectPaths};
use crate::support::sandbox::Live;

use super::{
    AGENT_HOME, destination_path, digest_in_sandbox, read_source, require_no_symlink_in_sandbox,
};

/// 実体の変更より先に保存する、1件のfile更新のintent。内容は保存しない。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PendingUpdate {
    version: u8,
    host: bool,
    source: String,
    destination: String,
    sha256: String,
    previous_sha256: Option<String>,
}

impl PendingUpdate {
    fn path(paths: &ProjectPaths) -> PathBuf {
        paths.sbxm_dir().join("file-update.yaml")
    }

    /// project lockの下で、fileを触る前に保存する。
    pub fn begin(
        paths: &ProjectPaths,
        file: &InitialProvisioningFile,
        previous: Option<String>,
        host: bool,
    ) -> Result<()> {
        Self::require_clean(paths)?;
        let pending = PendingUpdate {
            version: 1,
            host,
            source: file.source.clone(),
            destination: file.destination.clone(),
            sha256: file.sha256.clone(),
            previous_sha256: previous,
        };
        pending.validate(paths)?;
        let text = crate::config::serialized(&pending, "file-update.yaml")?;
        paths::atomic_create(&Self::path(paths), &text, PRIVATE_FILE_MODE)
    }

    /// 内容もpermissionも検証する。statusからも呼べる読み取りだけの入口。
    fn load(paths: &ProjectPaths) -> Result<Option<PendingUpdate>> {
        let path = Self::path(paths);
        if !paths::regular_file_exists(&path, PathScope::ProjectPath)? {
            return Ok(None);
        }
        let file = File::open(&path)
            .map_err(|error| paths::atomic_write_failed(&path, &error.to_string()))?;
        paths::require_private_file(&file, &path, PRIVATE_FILE_MODE, PathScope::ProjectPath)?;
        let open = paths::FileIdentity::of_open_file(&file);
        let current = paths::FileIdentity::of_path_without_following(&path);
        if !matches!((open, current), (Ok(a), Ok(b)) if a == b) {
            return Err(Self::pending_error(
                paths,
                msg!("cause-file-update-record-changed"),
            ));
        }
        let mut text = String::new();
        file.take(65_537)
            .read_to_string(&mut text)
            .map_err(|error| paths::atomic_write_failed(&path, &error.to_string()))?;
        if text.len() > 65_536 {
            return Err(Self::pending_error(
                paths,
                msg!("cause-file-update-record-invalid"),
            ));
        }
        let pending: Self = yaml_serde::from_str(&text)
            .map_err(|_| Self::pending_error(paths, msg!("cause-file-update-record-invalid")))?;
        pending.validate(paths)?;
        Ok(Some(pending))
    }

    fn validate(&self, paths: &ProjectPaths) -> Result<()> {
        let digest = |value: &str| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        };
        if self.version != 1
            || HostFileSource::new(&self.source).is_err()
            || SandboxHomeRelativePath::new(&self.destination).is_err()
            || !digest(&self.sha256)
            || self
                .previous_sha256
                .as_deref()
                .is_some_and(|value| !digest(value))
        {
            return Err(Self::pending_error(
                paths,
                msg!("cause-file-update-record-invalid"),
            ));
        }
        Ok(())
    }

    /// 未完了のfile更新を、別の構築や上書きで進めない。
    pub fn require_clean(paths: &ProjectPaths) -> Result<()> {
        match Self::load(paths)? {
            Some(_) => Err(Self::pending_error(
                paths,
                msg!("cause-file-update-record-pending"),
            )),
            None => Ok(()),
        }
    }

    fn pending_error(paths: &ProjectPaths, reason: crate::diagnostics::Msg) -> Error {
        Error::single(
            Diagnostic::new(
                ErrorId::FileUpdatePending,
                msg!("error-file-update-pending"),
            )
            .fact(Fact::path(&paths::display(&Self::path(paths))))
            .fact(Fact::reason(reason))
            .remediation(Remediation::text(msg!("remediation-file-update-pending"))),
        )
    }

    /// 更新後または再実行時に実体を照合し、baselineを先に保存してからintentを消す。
    pub fn recover(
        host: &dyn HostEnvironment,
        sandbox: &str,
        paths: &ProjectPaths,
        metadata: &mut ProjectMetadata,
    ) -> Result<()> {
        Self::recover_with(Some(host), sandbox, paths, metadata, metadata::update)
    }

    /// host側の採用はSandboxへの問い合わせを要しない。
    pub fn recover_host(paths: &ProjectPaths, metadata: &mut ProjectMetadata) -> Result<()> {
        Self::recover_with(None, "", paths, metadata, metadata::update)
    }

    fn recover_with(
        host: Option<&dyn HostEnvironment>,
        sandbox: &str,
        paths: &ProjectPaths,
        metadata: &mut ProjectMetadata,
        persist: impl FnOnce(&ProjectPaths, &ProjectMetadata) -> Result<()>,
    ) -> Result<()> {
        let Some(pending) = Self::load(paths)? else {
            return Ok(());
        };
        let observed = if pending.host {
            let source = Path::new(&pending.source);
            if paths::regular_file_exists(source, PathScope::ProjectPath)? {
                Some(read_source(source)?)
            } else {
                None
            }
        } else {
            let host = host.ok_or_else(|| {
                Self::pending_error(paths, msg!("cause-file-update-record-pending"))
            })?;
            let inside = Live::new(host, sandbox);
            let destination = destination_path(Path::new(&pending.destination))?;
            require_no_symlink_in_sandbox(&inside, Path::new(&pending.source), &destination)?;
            digest_in_sandbox(&inside, &format!("{AGENT_HOME}/{destination}"))?
        };
        if observed.as_deref() == Some(pending.sha256.as_str()) {
            let record = InitialProvisioningFile {
                source: pending.source.clone(),
                destination: pending.destination.clone(),
                sha256: pending.sha256.clone(),
            };
            let files = metadata.declared_files.get_or_insert_default();
            if !files.contains(&record) {
                let destination = SandboxHomeRelativePath::new(&pending.destination)
                    .map_err(|reason| Self::pending_error(paths, reason))?;
                files.retain(|file| {
                    SandboxHomeRelativePath::new(&file.destination)
                        .map_or(true, |old| !old.names_same_place(&destination))
                });
                files.push(record);
                persist(paths, metadata)?;
            }
        } else if observed != pending.previous_sha256 {
            return Err(Self::pending_error(
                paths,
                msg!("cause-file-update-target-changed"),
            ));
        }
        // baseline保存が失敗した場合はここへ届かず、次回も同じintentから再開できる。
        if Self::load(paths)?.as_ref() != Some(&pending) {
            return Err(Self::pending_error(
                paths,
                msg!("cause-file-update-record-changed"),
            ));
        }
        fs::remove_file(Self::path(paths))
            .map_err(|error| paths::atomic_write_failed(&Self::path(paths), &error.to_string()))
    }
}

#[cfg(test)]
#[path = "pending_update_test.rs"]
mod pending_update_test;
