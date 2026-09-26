/// `git push --porcelain`がref行の3つ目の欄に書く要約から、gitが示した理由を取り出す。
///
/// 要約は`[rejected] (non-fast-forward)`や`[remote rejected] (<理由>)`の形をとる。
/// 受け取る側が決める理由は、` (`や`)`を含みうる。先頭の`[...] (`と末尾の`)`だけを
/// 外す。この形でなければ、要約をそのまま返す。
pub fn refusal_reason(summary: &str) -> &str {
    summary
        .strip_prefix('[')
        .and_then(|rest| rest.split_once("] ("))
        .and_then(|(_, reason)| reason.strip_suffix(')'))
        .unwrap_or(summary)
}
