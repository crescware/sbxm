use std::path::Path;

use crate::boundary::host::HostEnvironment;
use crate::paths::ProjectPaths;
use crate::support::{disk::Storage, inventory::ProjectState};

use super::ProjectStatus;

/// host側の書き込み先と、稼働中Sandboxのroot・Docker dataを別々に診断する。
pub(super) fn check_storage(
    host: &dyn HostEnvironment,
    sandbox: &str,
    state: Option<ProjectState>,
    paths: &ProjectPaths,
    repository: &Path,
    status: &mut ProjectStatus,
) {
    status.storage = vec![
        Storage::host("storage-host-metadata", &paths.sbxm_dir()),
        Storage::host("storage-host-archives", &paths.cache_dir()),
        Storage::host("storage-host-backups", repository),
    ];
    if state == Some(ProjectState::Running) {
        let root = Storage::sandbox(host, sandbox, "storage-sandbox-root", "/");
        status.disk = root.usage.map_or(
            crate::support::disk::DiskObservation::ParseFailed,
            crate::support::disk::DiskObservation::Observed,
        );
        status.storage.push(root);
        status.storage.push(Storage::sandbox(
            host,
            sandbox,
            "storage-sandbox-docker",
            "/var/lib/docker",
        ));
    }
    for storage in &status.storage {
        if let Err(error) = storage.require_available() {
            status.diagnostics.extend_from_slice(error.diagnostics());
        }
    }
}
