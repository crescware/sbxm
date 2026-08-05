use crate::diagnostics::Result;

use super::SandboxReady;

/// sbxmが利用者の作業の代わりに起動することのないtool。設定と観測だけを行う。
pub trait Tool {
    /// Sandbox内でのcommand名。Dockerfileのmarker名にも使う。
    fn name(&self) -> &'static str;

    /// Sandboxが使える状態になったとき。`prepare`と`rebuild`が上げる。
    fn on_sandbox_ready(&self, ready: &mut SandboxReady) -> Result<()> {
        let _ = ready;
        Ok(())
    }
}
