use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;
use crate::project::SandboxName;

use crate::support::{generation, image};

use super::{ObservedGeneration, TargetSelection, changed_dockerfile_warning};

/// fresh案件の初回構築targetを、hostの変更前に一意に決める。
pub(crate) fn fresh_target(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    name: &SandboxName,
    metadata: &ProjectMetadata,
) -> Result<TargetSelection> {
    let stored = metadata.provisioning.dockerfile_sha256.clone();
    let current = generation::current_dockerfile_hash(paths)?;
    if current == stored {
        return Ok(TargetSelection {
            generation: stored,
            warnings: Vec::new(),
            stored: None,
        });
    }

    let built = image::generation_is_built(host, name, metadata.canonical_id(), &stored)?;
    let observed = Some(ObservedGeneration {
        dockerfile_sha256: stored.clone(),
        built,
    });
    if built {
        return Ok(TargetSelection {
            generation: stored,
            warnings: vec![changed_dockerfile_warning(metadata)],
            stored: observed,
        });
    }
    Ok(TargetSelection {
        generation: current,
        warnings: Vec::new(),
        stored: observed,
    })
}
