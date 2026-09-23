use std::path::PathBuf;

use crate::boundary::host::HostEnvironment;
use crate::diagnostics::Result;

use super::{AGENT_HOME, PlacedFile, Placement, copy_into_sandbox};

/// Sandboxを変えずに観測して決めた、1件の宣言の扱い。
///
/// すべての宣言の扱いを決めてから1件ずつ実行する。どれか1件でも置けないと分かれば、
/// 1件も置く前に拒否できる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    pub(super) index: usize,
    pub(super) source: PathBuf,
    /// `agent` homeからの相対path。
    pub(super) destination: String,
    /// `Placed`ならcopyし、`Unchanged`なら何もしない。
    pub(super) placement: Placement,
}

impl PlannedFile {
    /// 決めた扱いを実行する。
    pub fn carry_out(&self, host: &dyn HostEnvironment, sandbox: &str) -> Result<PlacedFile> {
        if self.placement == Placement::Placed {
            let full = format!("{AGENT_HOME}/{}", self.destination);
            copy_into_sandbox(host, sandbox, self.index, &self.source, &full)?;
        }
        Ok(PlacedFile {
            source: self.source.clone(),
            destination: self.destination.clone(),
            placement: self.placement,
        })
    }
}
