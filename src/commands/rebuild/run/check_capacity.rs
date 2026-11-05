use crate::boundary::host::HostEnvironment;
use crate::design::{Fact, ProgressSink};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::paths::ProjectPaths;
use crate::support::{
    disk::Storage, inventory, provisioning::SnapshotFile, repository, template::LoadedTemplate,
};

/// 古い環境を削除する前に、新しい環境の構築余力を確認する。
#[allow(clippy::too_many_arguments)]
pub(super) fn check_capacity(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    workspace_root: &std::path::Path,
    metadata: &ProjectMetadata,
    template: &LoadedTemplate,
    existed: bool,
    files: &[SnapshotFile],
    poll: inventory::Poll,
    progress: &mut dyn ProgressSink,
) -> Result<()> {
    let mut file_bytes = 0_u64;
    for file in files {
        let path = file.declaration.source.as_path();
        let size = std::fs::metadata(path)
            .map_err(|error| crate::paths::atomic_write_failed(path, &error.to_string()))?
            .len();
        file_bytes = file_bytes.saturating_add(size);
    }
    let host_required = file_bytes.div_ceil(1024).saturating_add(128 * 1024);
    for (label, path) in [
        ("storage-host-metadata", paths.sbxm_dir()),
        ("storage-host-metadata", workspace_root.to_path_buf()),
        ("storage-host-archives", paths.cache_dir()),
        (
            "storage-host-backups",
            repository::host_repository(paths, metadata),
        ),
    ] {
        require(&Storage::host(label, &path), host_required, 256)?;
    }
    let (kib, inodes) = if existed {
        super::space_required::space_required(host, metadata.sandbox_name().as_str(), metadata)?
    } else {
        (128 * 1024, 256)
    };
    let kib = kib.saturating_add(file_bytes.div_ceil(1024));
    super::capacity_probe::capacity_probe(host, workspace_root, template, poll, progress, |name| {
        require(
            &Storage::sandbox(host, name, "storage-sandbox-root", "/"),
            kib,
            inodes,
        )?;
        require(
            &Storage::sandbox(host, name, "storage-sandbox-docker", "/var/lib/docker"),
            1,
            1,
        )
    })
}

fn require(storage: &Storage, kib: u64, inodes: u64) -> Result<()> {
    storage.require_available()?;
    if storage
        .available_kib
        .is_some_and(|available| available < kib)
        || storage
            .available_inodes
            .is_some_and(|available| available < inodes)
    {
        return Err(Error::single(
            Diagnostic::new(
                ErrorId::RebuildCapacityInsufficient,
                msg!(
                    "error-rebuild-capacity-insufficient",
                    required = kib,
                    available = storage.available_kib.unwrap_or_default(),
                    inodes = inodes,
                    free_inodes = storage
                        .available_inodes
                        .map_or_else(|| "not-observed".to_string(), |value| value.to_string())
                ),
            )
            .fact(Fact::path(&storage.path))
            .remediation(msg!("remediation-rebuild-capacity-insufficient")),
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "check_capacity_test.rs"]
mod check_capacity_test;
