use console::Term;

/// 一覧に使える行数。端末の高さを読めない場合は制限しない。
pub(super) fn viewport(term: &Term) -> Option<usize> {
    if !term.is_term() {
        return None;
    }
    let (rows, _) = term.size();
    // heading、操作説明、選択数、空行、結果の一行ぶんを残す。
    Some((rows as usize).saturating_sub(6).max(1))
}
