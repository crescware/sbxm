use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;

use crate::support::image::BuiltImage;

use super::{LoadedTemplate, find};

/// 期待する名前のTemplateが既にあるか。
pub fn existing(host: &dyn HostEnvironment, image: &BuiltImage) -> Result<Option<LoadedTemplate>> {
    if find(host, &image.name)?.is_none() {
        return Ok(None);
    }
    Ok(Some(LoadedTemplate {
        name: image.name.clone(),
        loaded: false,
    }))
}
