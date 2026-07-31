//! runtimeが示すimageの偽の出力。

use crate::testing::outcome::{Checked, Required};

/// runtimeのimage storeが示す一覧。registry prefixを補って表示する。
pub fn template_listing(image: &str) -> Checked<String> {
    let (repository, tag) = image
        .rsplit_once(':')
        .required_because("an image reference")?;
    Ok(format!(
        r#"{{"images":[{{"id":"a3d0f4449170","repository":"docker.io/library/{repository}","tag":"{tag}"}}]}}"#
    ))
}
