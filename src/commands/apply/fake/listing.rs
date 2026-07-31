use crate::testing::outcome::Checked;

use std::path::Path;

use crate::project::SandboxName;
use crate::support::image::image_name;
use crate::testing::value::DIGEST;

use super::canonical;

pub fn listing(workspace_root: &Path, state: &str) -> Checked<String> {
    let name = SandboxName::derive(&canonical()?);
    Ok(format!(
        r#"[{{"name":"{name}","state":"{state}","workspace":"{}","template":"{}","active_sessions":0}}]"#,
        workspace_root.join(name.as_str()).display(),
        image_name(&name, DIGEST)
    ))
}
