//! runtimeが示すimageの偽の出力。

/// runtimeのimage storeが示す一覧。registry prefixを補って表示する。
pub fn template_listing(image: &str) -> String {
    let (repository, tag) = image.rsplit_once(':').expect("an image reference");
    format!(
        r#"{{"images":[{{"id":"a3d0f4449170","repository":"docker.io/library/{repository}","tag":"{tag}"}}]}}"#
    )
}
