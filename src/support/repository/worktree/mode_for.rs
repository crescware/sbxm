use crate::metadata::CreationMode;

/// これから作るworktreeのmode。
///
/// Gitは同じbranchを2つのworktreeへcheckoutさせない。attachedなworktreeは案件に1つしか
/// 持てないため、案件のmodeが効くのは最初の1本だけである。2本目以降はdetachedとする。
pub fn mode_for(index: u32, project: CreationMode) -> CreationMode {
    match index {
        0 => project,
        _ => CreationMode::Detached,
    }
}
