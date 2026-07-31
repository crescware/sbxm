/// 色判定が読む環境変数の観測値。
///
/// 値そのものではなく観測結果を持つことで、判定を純粋関数にする。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Environment {
    /// `NO_COLOR`が存在するか。値は問わず、空文字もopt-outとして扱う。
    pub no_color: bool,
    /// `CLICOLOR_FORCE`の値。
    pub clicolor_force: Option<String>,
    /// `TERM`の値。
    pub term: Option<String>,
}

impl Environment {
    /// 実行中のprocessの環境変数を読む。
    pub fn detect() -> Environment {
        Environment {
            no_color: std::env::var_os("NO_COLOR").is_some(),
            clicolor_force: std::env::var("CLICOLOR_FORCE").ok(),
            term: std::env::var("TERM").ok(),
        }
    }
}
