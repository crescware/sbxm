use super::{GITHUB_TOKEN_ENV, expected_token_env};

/// token環境変数fileが、sbxmが生成する3行だけで構成されているか。
///
/// markerの一致だけでは、利用者が後ろへ加えた設定までsbxmの所有物として切り詰めてしまう。
/// 以前のplaceholderを取り出して期待値を組み直し、末尾の改行を含めた全体が一致する場合だけ
/// sbxmが書いた形として扱う。
pub(super) fn is_sbxm_token_env(observed: &str) -> bool {
    let mut lines = observed.lines();
    let _marker = lines.next();
    let Some(placeholder) = lines
        .next()
        .and_then(|line| line.strip_prefix(&format!("export {GITHUB_TOKEN_ENV}=")))
    else {
        return false;
    };
    !placeholder.is_empty() && observed == expected_token_env(placeholder)
}
