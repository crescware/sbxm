use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::design::{Fact, ProgressSink};
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result, unparseable};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::paths::ProjectPaths;
use crate::support::{
    disk::Storage, inventory, provisioning::SnapshotFile, repository, sandbox,
    template::LoadedTemplate,
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
    let mut file_sizes = Vec::with_capacity(files.len());
    for file in files {
        let path = file.declaration.source.as_path();
        let size = std::fs::metadata(path)
            .map_err(|error| crate::paths::atomic_write_failed(path, &error.to_string()))?
            .len();
        file_bytes = file_bytes.saturating_add(size);
        file_sizes.push(size);
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
    super::capacity_probe::capacity_probe(host, workspace_root, template, poll, progress, |name| {
        let unit = allocation_unit(host, name)?;
        let (kib, inodes) = if existed {
            super::space_required::space_required(
                host,
                metadata.sandbox_name().as_str(),
                metadata,
                unit,
            )?
        } else {
            (128 * 1024, 256)
        };
        let (file_bytes, file_inodes) =
            files
                .iter()
                .zip(&file_sizes)
                .fold((0_u64, 0_u64), |(bytes, entries), (file, size)| {
                    let depth =
                        u64::try_from(file.declaration.destination.as_path().components().count())
                            .unwrap_or(u64::MAX);
                    (
                        bytes
                            .saturating_add(size.div_ceil(unit).saturating_mul(unit))
                            .saturating_add(depth.saturating_mul(unit)),
                        entries.saturating_add(depth),
                    )
                });
        let kib = kib.saturating_add(file_bytes.div_ceil(1024));
        let inodes = inodes.saturating_add(file_inodes);
        for (index, storage) in Storage::sandbox(
            host,
            name,
            &[
                ("storage-sandbox-root", "/"),
                ("storage-sandbox-docker", "/var/lib/docker"),
            ],
        )?
        .iter()
        .enumerate()
        {
            let (required_kib, required_inodes) = if index == 0 { (kib, inodes) } else { (1, 1) };
            require(storage, required_kib, required_inodes)?;
        }
        Ok(())
    })
}

/// 新環境のfilesystemが実際に割り当てる最小単位。試し書きは行わない。
fn allocation_unit(host: &dyn HostEnvironment, sandbox_name: &str) -> Result<u64> {
    let output = sandbox::exec_with_timeout(
        host,
        sandbox_name,
        &["stat", "-f", "-c", "%S", "/"],
        TimeoutClass::Probe,
    )?
    .require_success()?
    .stdout_text();
    output
        .trim()
        .parse::<u64>()
        .ok()
        .filter(|unit| *unit > 0)
        .ok_or_else(|| unparseable("stat -f -c %S", "missing or invalid allocation unit"))
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
