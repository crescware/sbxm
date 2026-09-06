use std::path::Path;

use crate::boundary::host::{HostEnvironment, TimeoutClass};
use crate::config::{ConfigLocation, GlobalConfig};
use crate::diagnostics::Result;
use crate::project::ProjectId;

use crate::support::provisioning::{self, NextAction};
use crate::support::{disk, select};

use crate::commands::status::project::artifacts::{check_directory, check_dockerfile, check_image};
use crate::commands::status::project::inside::{check_inside, check_sandbox};

use super::{ProjectStatus, Value};

/// 1案件を診断する。何も変更しない。
pub fn diagnose(
    location: &ConfigLocation,
    config: &GlobalConfig,
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
        next: None,
    };

    // 1. metadataと目標構成
    status.push("status-item-metadata", Value::Ready);

    // 2. project rootとhost clone
    check_directory(&paths, &mut status);

    // 3. Dockerfileの世代
    check_dockerfile(&paths, &metadata, &mut status);

    // 4. image、Sandbox
    check_image(host, &name, &metadata, &mut status);
    let state = check_sandbox(host, &metadata, workspace_root, &mut status);

    // 5-9. Sandbox内部の検査
    check_inside(host, &name, &metadata, state, &mut status);

    // root filesystemの使用量。running中だけ観測のためにcommandを実行する。
    status.disk = disk::observe(host, name.as_str(), state, TimeoutClass::Probe);

    // 次の1手は、`repair`と同じ共有観測から同じ規則で決める。statusが別の判定規則を
    // 持つと、案内したcommandが実行時に「不要」と答え得る。観測そのものが成立しない
    // 場合は、実行できると証明できないcommandを出さない。
    status.next = provisioning::observe(host, &paths, config, &metadata, workspace_root)
        .ok()
        .and_then(|observation| NextAction::decide(&metadata, &observation));

    Ok(status)
}
