use crate::hash::short_hex;
use crate::project::SandboxName;

/// `<sandbox-name>-template:<dockerfile-sha256-first-12-hex>`
pub fn image_name(sandbox: &SandboxName, dockerfile_sha256: &str) -> String {
    format!("{}-template:{}", sandbox, short_hex(dockerfile_sha256))
}
