use std::path::PathBuf;

use crate::metadata::CreationMode;

use crate::design::Warning;

/// `add`の結果。
///
/// 案件を管理下へ置き、host cloneを用意したところまでを示す。Sandboxはまだ存在せず、
/// 構築は`prepare`が行う。
#[derive(Debug, Clone)]
pub struct AddOutput {
    pub project: String,
    /// 構築で使うSandbox名。canonical project IDから決まり、この時点では未作成。
    pub sandbox: String,
    pub mode: CreationMode,
    /// 起点branch。attached modeは構築時にremoteから解決するため`None`のことがある。
    pub start_ref: Option<String>,
    pub requested_worktrees: u32,
    pub host_clone: PathBuf,
    /// 既に登録済みで、この実行が目標構成を変えなかったか。
    pub already_registered: bool,
    pub warnings: Vec<Warning>,
}
