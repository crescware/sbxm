use super::OPEN_FRAME_FIXED_ROWS;

/// promptを畳んだあとに残る結果の一行。
const RESULT_ROWS: usize = 1;

/// `open`の一覧に使える行数。画面の高さを読めない場合は制限しない。
///
/// 候補以外が使う行数は[`OPEN_FRAME_FIXED_ROWS`]が正本であり、ここでは数え直さない。
pub(super) fn open_viewport(rows: Option<u16>) -> Option<usize> {
    // 1行も残らない高さでも、選択中の候補だけは見せる。
    rows.map(|rows| {
        usize::from(rows)
            .saturating_sub(OPEN_FRAME_FIXED_ROWS + RESULT_ROWS)
            .max(1)
    })
}

#[cfg(test)]
#[path = "open_viewport_test.rs"]
mod open_viewport_test;
