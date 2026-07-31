use std::path::Path;

use crate::diagnostics::{Diagnostic, Error, ErrorId};
use crate::msg;

use crate::paths::inspect::{display, format_mode};

/// pathの用途。security messageの系列を選ぶ。
///
/// 同じ検査でも、対象がglobal設定なのか案件の成果物なのかで、利用者が取るべき
/// 対処が変わる。判定そのものは共通で、報告だけを用途ごとに分ける。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathScope {
    /// `~/.sbxm/config.yaml`のようなglobal設定file。
    ConfigFile,
    /// `~/.sbxm`のようなglobal設定directory。
    ConfigDir,
    /// 案件directory配下のfileとdirectory。
    ProjectPath,
}

impl PathScope {
    pub fn symlink_error(self, path: &Path) -> Error {
        let (id, description, remediation) = match self {
            PathScope::ConfigFile => (
                ErrorId::ConfigSymlink,
                "security-config-symlink-description",
                "security-config-symlink-remediation",
            ),
            PathScope::ConfigDir => (
                ErrorId::ConfigDirSymlink,
                "security-config-dir-symlink-description",
                "security-config-dir-symlink-remediation",
            ),
            PathScope::ProjectPath => (
                ErrorId::ProjectPathSymlink,
                "security-project-path-symlink-description",
                "security-project-path-symlink-remediation",
            ),
        };
        Error::single(
            Diagnostic::new(id, msg!(description, path = display(path)))
                .remediation(msg!(remediation, path = display(path))),
        )
    }

    pub fn owner_error(self, path: &Path, observed: u32, expected: u32) -> Error {
        let (id, description, remediation) = match self {
            PathScope::ConfigFile => (
                ErrorId::ConfigNotOwned,
                "security-config-owner-description",
                "security-config-owner-remediation",
            ),
            PathScope::ConfigDir => (
                ErrorId::ConfigDirNotOwned,
                "security-config-dir-owner-description",
                "security-config-dir-owner-remediation",
            ),
            PathScope::ProjectPath => (
                ErrorId::ProjectPathNotOwned,
                "security-project-path-owner-description",
                "security-project-path-owner-remediation",
            ),
        };
        Error::single(
            Diagnostic::new(
                id,
                msg!(description, path = display(path), observed = observed),
            )
            .remediation(msg!(remediation, path = display(path), expected = expected)),
        )
    }

    pub fn permission_error(self, path: &Path, observed: u32, expected: u32) -> Error {
        let (id, description, remediation) = match self {
            PathScope::ConfigFile => (
                ErrorId::ConfigPermissionTooOpen,
                "security-config-permission-description",
                "security-config-permission-remediation",
            ),
            PathScope::ConfigDir => (
                ErrorId::ConfigDirPermissionTooOpen,
                "security-config-dir-permission-description",
                "security-config-dir-permission-remediation",
            ),
            PathScope::ProjectPath => (
                ErrorId::ProjectFilePermissionTooOpen,
                "security-project-file-permission-description",
                "security-project-file-permission-remediation",
            ),
        };
        Error::single(
            Diagnostic::new(
                id,
                msg!(
                    description,
                    path = display(path),
                    observed = format_mode(observed)
                ),
            )
            .remediation(msg!(
                remediation,
                path = display(path),
                expected = format_mode(expected)
            )),
        )
    }

    pub fn unreadable_error(self, path: &Path, detail: &str) -> Error {
        match self {
            PathScope::ConfigFile | PathScope::ConfigDir => Error::new(
                ErrorId::ConfigUnreadable,
                msg!(
                    "error-config-unreadable",
                    path = display(path),
                    detail = detail
                ),
            ),
            PathScope::ProjectPath => Error::new(
                ErrorId::ProjectPathUnreadable,
                msg!(
                    "error-project-path-unreadable",
                    path = display(path),
                    detail = detail
                ),
            ),
        }
    }
}
