use super::Filter;

/// 一覧に使える行数。画面の高さを読めない場合は制限しない。
pub(super) fn viewport(rows: Option<u16>, filter: Filter) -> Option<usize> {
    // heading、空行、操作説明、選択数、空行、結果の一行ぶんを残す。絞り込める一覧は
    // 入力の一行も残す。1行も残らない高さでも、選択中の候補だけは見せる。
    let fixed = 6 + usize::from(filter == Filter::Typed);
    rows.map(|rows| usize::from(rows).saturating_sub(fixed).max(1))
}

#[cfg(test)]
#[path = "viewport_test.rs"]
mod viewport_test;
