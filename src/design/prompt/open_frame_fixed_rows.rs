/// `open`のpromptが候補の前後へ必ず置く行数。
///
/// heading、空行、操作説明、空行、絞り込み、worktree index、空行の7行。画面を組み立てる
/// `Painter::open_frame`と、一覧に割ける高さを決める`open_viewport`が同じ値を見る。
/// 一致する案件が無いときの警告は、候補が1行も無いときにだけ出るため数えない。
/// 画面の組み立てとの一致は`prompt_test`が確かめる。
pub(super) const OPEN_FRAME_FIXED_ROWS: usize = 7;
