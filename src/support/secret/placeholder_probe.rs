use super::GITHUB_TOKEN_ENV;

/// Sandboxの中でplaceholderを読むscript。
///
/// 未設定でも失敗させず空文字を返させる。`printenv`のexit statusで分けると、
/// 「設定されていない」と「読めなかった」を区別できない。
pub fn placeholder_probe() -> String {
    format!("printf %s \"${{{GITHUB_TOKEN_ENV}:-}}\"")
}
