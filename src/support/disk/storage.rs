use std::path::Path;

use crate::boundary::host::protocol::{RootDiskUsage, parse_df_single};
use crate::boundary::host::{CommandOutcome, HostEnvironment, TimeoutClass};
use crate::design::Fact;
use crate::diagnostics::{Diagnostic, Error, ErrorId, Result, unparseable};
use crate::support::sandbox;
use crate::{msg, paths};

/// 書き込み先ごとの空きblockとinode。試し書きは行わない。
#[derive(Debug, Clone)]
pub struct Storage {
    pub label: &'static str,
    pub path: String,
    pub available_kib: Option<u64>,
    pub available_inodes: Option<u64>,
    pub usage: Option<RootDiskUsage>,
    pub diagnostics: Vec<Diagnostic>,
}

impl Storage {
    fn empty(label: &'static str, path: String) -> Self {
        Self {
            label,
            path,
            available_kib: None,
            available_inodes: None,
            usage: None,
            diagnostics: Vec::new(),
        }
    }

    /// 未作成の保存先は最も近い既存ancestorのfilesystemで観測する。
    pub fn host(label: &'static str, path: &Path) -> Self {
        let mut storage = Self::empty(label, paths::display(path));
        let mut existing = path;
        let result = loop {
            match rustix::fs::statvfs(existing) {
                Ok(stat) => break Ok(stat),
                Err(rustix::io::Errno::NOENT) if existing.parent().is_some() => {
                    existing = existing.parent().unwrap_or(existing);
                }
                Err(error) => break Err(error),
            }
        };
        match result {
            Ok(stat) => {
                storage.available_kib = Some(stat.f_bavail.saturating_mul(stat.f_frsize) / 1024);
                storage.available_inodes = (stat.f_files > 0).then_some(stat.f_favail);
            }
            Err(error) => storage.diagnostics.push(
                storage
                    .problem(ErrorId::StorageUnobservable, "error-storage-unobservable")
                    .fact(Fact::cause(&error.to_string())),
            ),
        }
        storage.check_exhausted();
        storage
    }

    pub fn sandbox(
        host: &dyn HostEnvironment,
        sandbox: &str,
        label: &'static str,
        path: &str,
    ) -> Self {
        let mut storage = Self::empty(label, path.into());
        let blocks =
            sandbox::exec_with_timeout(host, sandbox, &["df", "-Pk", path], TimeoutClass::Probe)
                .and_then(CommandOutcome::require_success)
                .and_then(|outcome| parse_df_single(&outcome.stdout_text()));
        storage.usage = storage.record(blocks);
        storage.available_kib = storage.usage.map(|usage| usage.free_kib);
        let inodes =
            sandbox::exec_with_timeout(host, sandbox, &["df", "-Pi", path], TimeoutClass::Probe)
                .and_then(CommandOutcome::require_success)
                .and_then(|outcome| inode_count(&outcome.stdout_text()));
        storage.available_inodes = storage.record(inodes);
        storage.check_exhausted();
        storage
    }

    fn record<T>(&mut self, result: Result<T>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.diagnostics.extend_from_slice(error.diagnostics());
                None
            }
        }
    }

    fn problem(&self, id: ErrorId, message: &'static str) -> Diagnostic {
        Diagnostic::new(id, msg!(message))
            .fact(Fact::path(&self.path))
            .remediation(msg!("remediation-storage-capacity"))
    }

    pub fn exhausted(&self) -> bool {
        self.available_kib == Some(0) || self.available_inodes == Some(0)
    }

    fn check_exhausted(&mut self) {
        if self.exhausted() {
            self.diagnostics
                .push(self.problem(ErrorId::StorageExhausted, "error-storage-exhausted"));
        }
    }

    /// 観測不能を成功と扱わない、書き込み前の最小限の確認。
    pub fn require_available(&self) -> Result<()> {
        if !self.diagnostics.is_empty() {
            return Err(Error::Diagnostics(self.diagnostics.clone()));
        }
        if self.available_kib.is_none() {
            return Err(Error::single(self.problem(
                ErrorId::StorageUnobservable,
                "error-storage-unobservable",
            )));
        }
        Ok(())
    }
}

fn inode_count(output: &str) -> Result<u64> {
    let mut lines = output.lines().filter(|line| !line.trim().is_empty());
    let header = lines.next().unwrap_or_default();
    let row = lines
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>();
    if !header.contains("IFree") || row.len() < 6 || lines.next().is_some() {
        return Err(unparseable("df -Pi", "missing or ambiguous inode counters"));
    }
    row[3]
        .parse()
        .map_err(|_| unparseable("df -Pi", "invalid available inode counter"))
}

#[cfg(test)]
#[path = "storage_test.rs"]
mod storage_test;
