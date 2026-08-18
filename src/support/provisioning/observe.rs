use std::path::Path;

use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::hash::short_hex;
use crate::metadata::ProjectMetadata;
use crate::paths::{self, PathScope, ProjectPaths};
use crate::project::{SandboxLayout, SandboxName};

use crate::support::{daemon, generation, image, inventory, sandbox, template};

use super::{Artifact, Observation, ProvisioningState, already_built};

/// metadataと可変成果物から、初回構築の状態を毎回算出する。
pub fn observe(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    name: &SandboxName,
    metadata: &ProjectMetadata,
    layout: &SandboxLayout,
    workspace_root: &Path,
    inspect_pending: bool,
) -> Result<Observation> {
    if metadata.initial_provisioning.is_some() && !inspect_pending {
        return Ok(Observation {
            state: ProvisioningState::Pending,
            output: None,
            artifacts: vec![Artifact::Sandbox],
        });
    }

    if metadata.initial_provisioning.is_none()
        && let Some(output) = already_built(host, paths, name, metadata, layout, workspace_root)?
    {
        return Ok(Observation {
            state: ProvisioningState::Ready,
            output: Some(output),
            artifacts: Vec::new(),
        });
    }

    let artifacts = incomplete_artifacts(host, paths, name, metadata, workspace_root)?;
    Ok(Observation {
        state: if metadata.initial_provisioning.is_some() {
            ProvisioningState::Pending
        } else if artifacts.is_empty() {
            ProvisioningState::Fresh
        } else {
            ProvisioningState::Incomplete
        },
        output: None,
        artifacts,
    })
}

/// 初回構築の途中に残った成果物をread-onlyで列挙する。
fn incomplete_artifacts(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    name: &SandboxName,
    metadata: &ProjectMetadata,
    workspace_root: &Path,
) -> Result<Vec<Artifact>> {
    let mut artifacts = Vec::new();
    let entries = daemon::list(host)?;
    if let Some(entry) = inventory::single(&entries, name.as_str())? {
        sandbox::verify_identity(entry, name, workspace_root)?;
        artifacts.push(Artifact::Sandbox);
    }
    if sandbox::workspace_exists(workspace_root, name)? {
        artifacts.push(Artifact::Workspace);
    }

    let stored = metadata.provisioning.dockerfile_sha256.clone();
    let current = generation::current_dockerfile_hash(paths)?;
    let generations = if stored == current {
        vec![stored]
    } else {
        vec![stored, current]
    };
    for generation in generations {
        let image_name = image::image_name(name, &generation);
        if image::generation_is_built(host, name, metadata.canonical_id(), &generation)? {
            artifacts.push(Artifact::Image(image_name.clone()));
        }
        if template::has(host, &image_name)? {
            artifacts.push(Artifact::Template(image_name.clone()));
        }
        let short = short_hex(&generation);
        for (path, label) in [
            (paths.template_archive(short), "archive"),
            (paths.template_archive_temp(short), "temporary archive"),
        ] {
            if paths::regular_file_exists(&path, PathScope::ProjectPath)? {
                artifacts.push(Artifact::Archive(format!(
                    "{} ({label})",
                    paths::display(&path)
                )));
            }
        }
    }
    Ok(artifacts)
}
