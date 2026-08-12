use crate::testing::outcome::Checked;

use std::path::Path;

use crate::project::SandboxName;

use super::canonical;

pub fn listing(workspace_root: &Path, state: &str) -> Checked<String> {
    let name = SandboxName::derive(&canonical()?);
    Ok(format!(
        r#"{{"sandboxes":[{{"name":"{name}","status":"{state}","workspaces":["{}"]}}]}}"#,
        workspace_root.join(name.as_str()).display(),
    ))
}
