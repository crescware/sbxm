use crate::config::GlobalConfig;
use crate::diagnostics::Result;
use crate::metadata::{InitialProvisioningFile, InitialProvisioningIntent};
use crate::paths;
use crate::support::files;

/// 最初のmutationの前に保存する、初回構築の固定intentを作る。
pub(crate) fn initial_intent(
    config: &GlobalConfig,
    target_dockerfile_sha256: &str,
) -> Result<InitialProvisioningIntent> {
    let mut files_snapshot = Vec::with_capacity(config.files.len());
    for declaration in &config.files {
        files_snapshot.push(InitialProvisioningFile {
            source: paths::display(declaration.source.as_path()),
            destination: paths::display(declaration.destination.as_path()),
            sha256: files::read_source(declaration.source.as_path())?,
        });
    }
    Ok(InitialProvisioningIntent {
        target_dockerfile_sha256: target_dockerfile_sha256.to_string(),
        files: files_snapshot,
    })
}
