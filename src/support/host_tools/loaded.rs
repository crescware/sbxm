use std::collections::BTreeMap;

use super::RecordedTool;

/// 記録fileを読んだ結果。
pub(super) enum Loaded {
    /// fileが無い。
    Missing,
    /// 要件を満たさないfile。symlink、通常file以外、他人所有、開きすぎたpermission。
    /// 使わず、所有を確かめられない成果物として置き換えもしない。
    Unusable,
    /// 読めない、または解釈できないfile。使わず、確認が通れば置き換える。
    Corrupt,
    Records(BTreeMap<String, RecordedTool>),
}
