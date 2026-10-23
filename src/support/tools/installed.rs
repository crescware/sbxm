use std::collections::BTreeSet;

use crate::diagnostics::Result;

use crate::support::sandbox::Inside;

use super::{ALL, Tool, probe_command};

/// Sandboxが持っているtool。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Installed(pub(crate) BTreeSet<&'static str>);

impl Installed {
    /// Sandboxが持っているtoolを、1回の起動で数える。
    pub fn observe(inside: &dyn Inside) -> Result<Installed> {
        let command = probe_command();
        let borrowed: Vec<&str> = command.iter().map(String::as_str).collect();
        let outcome = inside.exec(&borrowed)?.require_success()?;
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
