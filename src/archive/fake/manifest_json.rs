/// `manifest.json`の内容。
pub fn manifest_json(image_name: &str, image_id: &str) -> String {
    let hex = image_id.strip_prefix("sha256:").unwrap_or(image_id);
    format!(r#"[{{"Config":"blobs/sha256/{hex}","RepoTags":["{image_name}"],"Layers":[]}}]"#)
}
