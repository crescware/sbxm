use crate::boundary::host::HostEnvironment;
use crate::config::{GlobalConfig, SandboxHomeRelativePath};
use crate::metadata::ProjectMetadata;
use crate::paths;
use crate::project::SandboxName;
use crate::support::files;
use crate::support::inventory::ProjectState;

use super::{FileRow, FileState, ProjectStatus};

/// 現在の宣言fileを、sbxmがこの案件へ最後に置いた内容と比べる。
///
/// hostでは宣言したfileが変わったかを、Sandboxでは中で書き換えられたかを見る。どちらも
/// 何も変更しない。Sandboxは動いている場合だけ読み、停止中のSandboxを起動しない。
pub fn check_files(
    host: &dyn HostEnvironment,
    name: &SandboxName,
    metadata: &ProjectMetadata,
    config: &GlobalConfig,
    state: Option<ProjectState>,
    status: &mut ProjectStatus,
) {
    let baseline = metadata.declared_files.as_deref().unwrap_or_default();
    for declared in &config.files {
        let recorded = baseline
            .iter()
            .find(|entry| {
                SandboxHomeRelativePath::new(&entry.destination)
                    .is_ok_and(|placed| placed.names_same_place(&declared.destination))
            })
            .map(|entry| entry.sha256.as_str());

        let current = match files::read_source(declared.source.as_path()) {
            Ok(digest) => Some(digest),
            Err(error) => {
                // `apply --files`がこの宣言で止まる。止まる理由を先に示す。
                status
                    .diagnostics
                    .extend(error.diagnostics().iter().cloned());
                None
            }
        };
        let host_state = match (current.as_deref(), recorded) {
            (None, _) => FileState::Unreadable,
            (Some(_), None) => FileState::Unplaced,
            (Some(current), Some(recorded)) if current == recorded => FileState::Unchanged,
            (Some(_), Some(_)) => FileState::Updated,
        };

        let sandbox_state = match state {
            Some(ProjectState::NotCreated) => FileState::NotApplicable,
            // read-onlyの検査でもSandboxを起動し得るため実行しない。
            Some(ProjectState::Stopped) => FileState::NotObservedStopped,
            None => FileState::NotObserved,
            Some(ProjectState::Running) => match files::sandbox_digest(
                host,
                name.as_str(),
                declared.source.as_path(),
                declared.destination.as_path(),
            ) {
                Err(error) => {
                    status
                        .diagnostics
                        .extend(error.diagnostics().iter().cloned());
                    FileState::NotObserved
                }
                Ok(None) => FileState::Missing,
                Ok(Some(held)) => match recorded {
                    Some(recorded) if held == recorded => FileState::Unchanged,
                    Some(_) => FileState::Modified,
                    None if current.as_deref() == Some(held.as_str()) => FileState::Unchanged,
                    None => FileState::Unrecorded,
                },
            },
        };

        status.files.push(FileRow {
            destination: paths::display(declared.destination.as_path()),
            host: host_state,
            sandbox: sandbox_state,
        });
    }
}

#[cfg(test)]
#[path = "check_files_test.rs"]
mod check_files_test;
