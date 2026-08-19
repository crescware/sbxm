use crate::command::HostEnvironment;
use crate::design::Warning;
use crate::diagnostics::Result;
use crate::metadata::{self, ProjectMetadata};
use crate::paths::ProjectPaths;
use crate::project::SandboxName;

use crate::support::{generation, image};

use super::changed_dockerfile_warning::changed_dockerfile_warning;

/// 初回構築を完成させる世代を決める。
///
/// image buildの前にDockerfileが変わった場合は、現在のDockerfileを目標とする。
/// 既にimageがある場合は保存済み世代で完成させ、現在の内容は`rebuild`へ案内する。
pub(crate) fn fresh_target(
    host: &dyn HostEnvironment,
    paths: &ProjectPaths,
    metadata: &mut ProjectMetadata,
    name: &SandboxName,
) -> Result<(String, Vec<Warning>)> {
    let stored = metadata.provisioning.dockerfile_sha256.clone();
    let current = generation::current_dockerfile_hash(paths)?;
    if current == stored {
        return Ok((stored, Vec::new()));
    }

    if image::generation_is_built(host, name, metadata.canonical_id(), &stored)? {
        // 注意だけを出して終えない。現在のDockerfileを適用する手順まで示す。
        return Ok((stored, vec![changed_dockerfile_warning(metadata)]));
    }

    metadata.provisioning.dockerfile_sha256.clone_from(&current);
    metadata::update(paths, metadata)?;
    Ok((current, Vec::new()))
}
