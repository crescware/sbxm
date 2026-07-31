use crate::config::ConfigLocation;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::paths::{ExclusiveLock, ProjectPaths};
use crate::project::ProjectId;

use super::find;

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
    /// 完全指定された案件をlockし、lock後のmetadataとともに返す。
    ///
    /// promptを持たないcommandの入口。`load`が先に存在を確かめるため、管理対象でない
    /// 案件にはlock fileを作らない。
    pub fn acquire(location: &ConfigLocation, project: &ProjectId) -> Result<Locked> {
        find(location, project)?.lock()
    }
}
