use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;
use crate::project::{CanonicalProjectId, SandboxName};

use super::{expected_labels, image_name, inspect, labels_match};

/// 世代に対応するimageが既にあるか。
///
/// 初回構築の途中でDockerfileが変わった場合に、どちらの世代で完成させるかを決める。
pub fn generation_is_built(
    host: &dyn HostEnvironment,
    sandbox: &SandboxName,
    canonical: &CanonicalProjectId,
    dockerfile_sha256: &str,
) -> Result<bool> {
    let name = image_name(sandbox, dockerfile_sha256);
    let labels = expected_labels(canonical, dockerfile_sha256);
    Ok(inspect(host, &name)?.is_some_and(|identity| labels_match(&identity, &labels)))
}
