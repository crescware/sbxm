use crate::testing::outcome::{Checked, Required};

/// runtimeが`id`で報告するTemplateが1件だけある一覧。registry prefixを補って表示する。
pub fn template_listing_with_id(image: &str, id: &str) -> Checked<String> {
    let (repository, tag) = image
        .rsplit_once(':')
        .required_because("an image reference")?;
    Ok(format!(
        r#"{{"images":[{{"id":"{id}","repository":"docker.io/library/{repository}","tag":"{tag}"}}]}}"#
    ))
}
