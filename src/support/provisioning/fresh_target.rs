use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::paths::ProjectPaths;
use crate::project::SandboxName;

use crate::support::{generation, image};

use super::changed_dockerfile_warning;

/// fresh案件の初回構築targetを、hostの変更前に一意に決める。
pub(crate) fn fresh_target(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    name: &SandboxName,
    metadata: &ProjectMetadata,
) -> Result<(String, Vec<crate::design::Warning>)> {
    let stored = metadata.provisioning.dockerfile_sha256.clone();
    let current = generation::current_dockerfile_hash(paths)?;
    if current == stored {
        return Ok((stored, Vec::new()));
    }
    if image::generation_is_built(host, name, metadata.canonical_id(), &stored)? {
        return Ok((stored, vec![changed_dockerfile_warning(metadata)]));
    }
    Ok((current, Vec::new()))
}
