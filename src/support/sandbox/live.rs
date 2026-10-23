use crate::boundary::host::{CommandOutcome, HostEnvironment};
use crate::diagnostics::Result;

use super::{Inside, exec};

/// commandごとに`sbx exec`を起動して答える、Sandboxの中。
pub struct Live<'a> {
    host: &'a dyn HostEnvironment,
    sandbox: &'a str,
}

impl<'a> Live<'a> {
    pub fn new(host: &'a dyn HostEnvironment, sandbox: &'a str) -> Live<'a> {
        Live { host, sandbox }
    }
}

impl Inside for Live<'_> {
    fn exec(&self, args: &[&str]) -> Result<CommandOutcome> {
        exec(self.host, self.sandbox, args)
    }
}
