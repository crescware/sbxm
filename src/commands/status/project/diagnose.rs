use std::path::Path;

use crate::command::{HostEnvironment, TimeoutClass};
use crate::config::ConfigLocation;
use crate::diagnostics::Result;
use crate::project::ProjectId;

use crate::support::{disk, select};

use crate::commands::status::project::artifacts::{
    check_archive, check_directory, check_dockerfile, check_image,
};
use crate::commands::status::project::inside::{check_inside, check_sandbox};

use super::{ProjectStatus, Value};

/// 1案件を診断する。何も変更しない。
pub fn diagnose(
    location: &ConfigLocation,
    project: &ProjectId,
    host: &dyn HostEnvironment,
    workspace_root: &Path,
) -> Result<ProjectStatus> {
    // 案件の場所はregistryだけが持つ。配置規則から再計算しない。
    let candidate = select::find(location, project)?;
    let paths = candidate.paths.clone();
    let metadata = candidate.reload()?;
    let name = metadata.sandbox_name();

    let mut status = ProjectStatus {
        project: metadata.display_id(),
        items: Vec::new(),
        worktrees: Vec::new(),
        disk: disk::DiskObservation::NotObservedMismatch,
        diagnostics: Vec::new(),
    };

    // 1. metadataと目標構成
    status.push("status-item-metadata", Value::Ready);

    // 2. project rootとhost clone
    check_directory(&paths, &mut status);

    // 3. Dockerfileの世代
    check_dockerfile(&paths, &metadata, &mut status);

    // 4-5. image、archive、Sandbox
    check_image(host, &name, &metadata, &mut status);
    check_archive(&paths, &metadata, &mut status);
    let state = check_sandbox(host, &metadata, workspace_root, &mut status);

    // 6-10. Sandbox内部の検査
    check_inside(host, &name, &metadata, state, &mut status);

    // root filesystemの使用量。running中だけ観測のためにcommandを実行する。
    status.disk = disk::observe(host, name.as_str(), state, TimeoutClass::SandboxLifecycle);

    Ok(status)
}
