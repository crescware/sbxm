use std::collections::BTreeSet;

use crate::command::HostEnvironment;
use crate::diagnostics::Result;

use crate::support::sandbox;

use super::{ALL, Tool, probe};

/// Sandboxが持っているtool。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Installed(pub(crate) BTreeSet<&'static str>);

impl Installed {
    /// Sandboxが持っているtoolを、1回の起動で数える。
    pub fn observe(host: &dyn HostEnvironment, sandbox: &str) -> Result<Installed> {
        let outcome = sandbox::exec(host, sandbox, &["sh", "-c", &probe()])?.require_success()?;
        let answer = outcome.stdout_text();
        let named: Vec<&str> = answer.lines().map(str::trim).collect();
        Ok(Installed(
            ALL.iter()
                .map(|tool| tool.name())
                .filter(|name| named.contains(name))
                .collect(),
        ))
    }

    pub fn has(&self, tool: &dyn Tool) -> bool {
        self.0.contains(tool.name())
    }
}
