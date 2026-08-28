/// `boundary::terminal`が観測した環境変数。
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
