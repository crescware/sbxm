//! `project.toml`の書き出し。

use super::{METADATA_VERSION, ProjectMetadata};

/// metadataをTOMLへ描画する。
pub fn render(metadata: &ProjectMetadata) -> String {
    let mut out = String::new();
    out.push_str(&format!("version = {METADATA_VERSION}\n"));
    out.push_str(&format!("owner = {}\n", toml_string(&metadata.owner)));
    out.push_str(&format!(
        "repository = {}\n",
        toml_string(&metadata.repository)
    ));
    out.push_str(&format!(
        "canonical_id = {}\n",
        toml_string(metadata.canonical_id.as_str())
    ));

    let provisioning = &metadata.provisioning;
    out.push_str("\n[provisioning]\n");
    out.push_str(&format!(
        "mode = {}\n",
        toml_string(provisioning.mode.as_str())
    ));
    out.push_str(&format!(
        "start_ref = {}\n",
        toml_string(provisioning.start_ref.as_deref().unwrap_or(""))
    ));
    out.push_str(&format!(
        "requested_worktrees = {}\n",
        provisioning.requested_worktrees
    ));
    out.push_str(&format!(
        "dockerfile_sha256 = {}\n",
        toml_string(&provisioning.dockerfile_sha256)
    ));

    if let Some(rebuild) = &metadata.rebuild {
        out.push_str("\n[rebuild]\n");
        out.push_str(&format!(
            "target_dockerfile_sha256 = {}\n",
            toml_string(&rebuild.target_dockerfile_sha256)
        ));
        out.push_str(&format!(
            "previous_dockerfile_sha256 = {}\n",
            toml_string(&rebuild.previous_dockerfile_sha256)
        ));
    }

    out
}

pub(super) fn toml_string(value: &str) -> String {
    toml::Value::String(value.to_string()).to_string()
}
