/// `open`のpromptが候補の前後へ必ず置く行数。
///
/// heading、空行、操作説明、空行、worktree index、空行の6行。画面を組み立てる
/// `Painter::open_frame`と、一覧に割ける高さを決める`open_viewport`が同じ値を見る。
/// 画面の組み立てとの一致は`prompt_test`が確かめる。
pub(super) const OPEN_FRAME_FIXED_ROWS: usize = 6;
