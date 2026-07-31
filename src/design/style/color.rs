/// ANSI標準16色のうち、意味色として使う4色。
///
/// magentaと背景色を持たないのは、terminal themeとの衝突が大きく、意味の数に対して
/// 視覚的な負荷が高いためである。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    Red,
    Green,
    Yellow,
    Cyan,
}
