use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;

use crate::support::sandbox::Live;

use super::{ALL, Installed};

/// Sandboxが使える状態になった瞬間。
pub struct SandboxReady<'a> {
    pub host: &'a dyn HostEnvironment,
    pub sandbox: &'a str,
}

impl SandboxReady<'_> {
    /// Sandboxが使える状態になったことを、入っているtoolへ伝える。
    pub fn announce(host: &dyn HostEnvironment, sandbox: &str) -> Result<()> {
        let installed = Installed::observe(&Live::new(host, sandbox))?;
        let mut ready = SandboxReady { host, sandbox };
        for tool in ALL {
            if installed.has(tool) {
                tool.on_sandbox_ready(&mut ready)?;
            }
        }
        Ok(())
    }
}
