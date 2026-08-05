use std::path::Path;

use crate::command::{HostEnvironment, TimeoutClass};
use crate::diagnostics::Result;
use crate::msg;
use crate::paths;

use crate::design::ProgressSink;
use crate::support::image::BuiltImage;
use crate::support::sandbox;

use super::{LoadedTemplate, find, unusable};

/// imageに対応するTemplateを用意する。
///
/// 期待する名前のTemplateが既にあれば再利用する。名前には世代のDockerfile hashが
/// 入るため、別世代のTemplateを取り違えることはない。load直後は、その名前が一覧に
/// 現れたことを確かめる。現れない場合はloadの成功を推測しない。
pub fn ensure(
    host: &dyn HostEnvironment,
    archive: &Path,
    image: &BuiltImage,
    progress: &mut dyn ProgressSink,
) -> Result<LoadedTemplate> {
    if find(host, &image.name)?.is_some() {
        return Ok(LoadedTemplate {
            name: image.name.clone(),
            loaded: false,
        });
    }

    progress.step(msg!("progress-loading-template"));
    let command = sandbox::relayed(&["template", "load", &paths::display(archive)])
        .timeout(TimeoutClass::SandboxLifecycle);
    host.run_with_terminal(&command, progress)?
        .require_success()?;

    if find(host, &image.name)?.is_none() {
        return Err(unusable(
            &image.name,
            msg!("cause-template-absent-after-load"),
        ));
    }

    Ok(LoadedTemplate {
        name: image.name.clone(),
        loaded: true,
    })
}
