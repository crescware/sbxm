use crate::design::ProgressSink;
use crate::diagnostics::Result;
use crate::metadata::ProjectMetadata;
use crate::support::files::{self, PlacedFile};
use crate::support::sandbox::Inside;

/// 宣言fileの現在状態を、復旧すべきbaselineと照らして観測する。
///
/// 対象はintentが固定したsnapshot、それが無ければ完成時に残したbaselineであり、
/// 現在のglobal configそのものではない。completed projectへ後から宣言を足しても、
/// この観測はそれを欠落として報告しない。新しい宣言の配置は`apply`の責務である。
///
/// baselineはsbxmが最後に置いた内容であり、それと異なるfileはSandboxの中で書き換え
/// られたものとして報告する。利用者が編集した設定fileは壊れた成果物ではない。intentの
/// snapshotはこれから置く内容であり、異なるfileはどこから来たか分からないため拒否する。
///
/// intentもbaselineも無い案件には、sbxmが置いたと記録した宣言fileが無い。空のbaseline
/// として扱い、何も観測しない。
pub(crate) fn declared_files(
    inside: &dyn Inside,
    metadata: &ProjectMetadata,
    progress: &mut dyn ProgressSink,
) -> Result<Vec<PlacedFile>> {
    if let Some(intent) = &metadata.initial_provisioning {
        return files::observe_against_baseline(
            inside,
            &intent.files,
            files::Divergence::Conflict,
            progress,
        );
    }
    files::observe_against_baseline(
        inside,
        metadata.declared_files.as_deref().unwrap_or_default(),
        files::Divergence::Modified,
        progress,
    )
}
