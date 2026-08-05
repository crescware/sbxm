/// 一覧に使える行数。画面の高さを読めない場合は制限しない。
pub(super) fn viewport(rows: Option<u16>) -> Option<usize> {
    // heading、操作説明、選択数、空行、結果の一行ぶんを残す。
    // 1行も残らない高さでも、選択中の候補だけは見せる。
    rows.map(|rows| usize::from(rows).saturating_sub(6).max(1))
}

#[cfg(test)]
#[path = "viewport_test.rs"]
mod viewport_test;
