use crate::diagnostics::{Diagnostic, Error, ErrorId, Result};
use crate::metadata::ProjectMetadata;
use crate::msg;
use crate::paths::{
    ExclusiveLock, LOCK_TIMEOUT, PRIVATE_FILE_MODE, PathScope, ProjectPaths, SharedLock,
};
use crate::paths::{acquire_exclusive_lock, acquire_shared_lock};

/// project lockを保持したまま読み直した1案件。
///
/// 選択の時点で読んだmetadataは古くなり得る。判定はlock後の内容だけで行う。
/// 管理対象でない案件にはlock fileを作らない。
///
/// 読み直しの時点でmetadataが消えていた場合、案件名は引数の綴りではなく、
/// 保存されていたmetadataの綴りで報告する。promptで選んだ場合は引数が存在しないため、
/// 読み直しの経路は1つに保つ。
///
/// lockは値の生存期間だけ有効である。分解するとその場でlockが外れるため、
/// fieldは`locked.paths`・`locked.metadata`として使う。
#[derive(Debug)]
pub struct Locked {
    pub paths: ProjectPaths,
    pub metadata: ProjectMetadata,
    pub(super) _lock: ExclusiveLock,
}

impl Locked {
    /// `sbxm open`が保持するshared session lease。
    ///
    /// project lockを保持している`Locked`のmethodとしてしか呼べないことで、
    /// lock順序をproject lock→session leaseに固定する。sharedなので、複数の
    /// `sbxm open` sessionが同じ案件で共存してもここでは待たされない。
    pub fn acquire_shared_session_lease(&self) -> Result<SharedLock> {
        acquire_shared_lock(
            &self.paths.session_lease_file(),
            LOCK_TIMEOUT,
            PRIVATE_FILE_MODE,
            PathScope::ProjectPath,
        )
    }

    /// 通常rebuild/destroyが保持するexclusive session lease。
    ///
    /// project lockを保持している`Locked`のmethodとしてしか呼べないことで、
    /// lock順序をproject lock→session leaseに固定する。呼び出し時点でこの案件の
    /// project lockは自分が排他的に保持しているため、取得できない原因は開いている
    /// `sbxm open` sessionのshared leaseだけである。
    pub fn acquire_exclusive_session_lease(&self) -> Result<ExclusiveLock> {
        acquire_exclusive_lock(
            &self.paths.session_lease_file(),
            LOCK_TIMEOUT,
            PRIVATE_FILE_MODE,
            PathScope::ProjectPath,
        )
        .map_err(|error| {
            if error.contains_id(ErrorId::LockTimeout) {
                open_session_active(&self.metadata.display_id())
            } else {
                error
            }
        })
    }
}

/// 開いているsessionが原因で、通常のrebuild/destroyがexclusive session leaseを取得できない。
fn open_session_active(project: &str) -> Error {
    Error::single(
        Diagnostic::new(
            ErrorId::OpenSessionActive,
            msg!("error-open-session-active", project = project),
        )
        .remediation(msg!("remediation-open-session-active")),
    )
}
