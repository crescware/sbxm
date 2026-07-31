use crate::diagnostics::Result;

use crate::support::identity;

use super::{SandboxReady, Tool};

/// GitHub CLI。
pub struct Gh;

impl Tool for Gh {
    fn name(&self) -> &'static str {
        "gh"
    }

    fn on_sandbox_ready(&self, ready: &mut SandboxReady) -> Result<()> {
        identity::ensure_git_protocol(ready.host, ready.sandbox)
    }
}
