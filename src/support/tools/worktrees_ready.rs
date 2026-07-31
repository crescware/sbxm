use crate::command::HostEnvironment;
use crate::diagnostics::Result;
use crate::project::SandboxLayout;

use super::{ALL, Installed, Note};

/// managed worktreeが揃った瞬間。
pub struct WorktreesReady<'a> {
    pub host: &'a dyn HostEnvironment,
    pub sandbox: &'a str,
    pub layout: &'a SandboxLayout,
    pub count: usize,
    pub notes: &'a mut Vec<Note>,
}

impl WorktreesReady<'_> {
    /// managed worktreeが揃ったことを、入っているtoolへ伝える。
    pub fn announce(
        host: &dyn HostEnvironment,
        sandbox: &str,
        layout: &SandboxLayout,
        count: usize,
    ) -> Result<Vec<Note>> {
        let installed = Installed::observe(host, sandbox)?;
        let mut notes = Vec::new();
        let mut ready = WorktreesReady {
            host,
            sandbox,
            layout,
            count,
            notes: &mut notes,
        };
        for tool in ALL {
            if installed.has(tool) {
                tool.on_worktrees_ready(&mut ready)?;
            }
        }
        Ok(notes)
    }
}
