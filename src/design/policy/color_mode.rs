/// `--color`が受け取る三値。
///
/// 値の一覧と表記をここだけが持つ。parserもhelpもこの宣言から導出する。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ColorMode {
    /// streamがTTYのときだけ色を出す。
    #[default]
    Auto,
    /// redirect先にも色を出す。利用者が明示した場合だけ選べる。
    Always,
    /// 色を出さない。
    Never,
}

impl ColorMode {
    /// 受け付ける値の全体。helpの一覧もparserの候補もここから作る。
    pub const ALL: [ColorMode; 3] = [ColorMode::Auto, ColorMode::Always, ColorMode::Never];

    /// 翻訳しない安定した表記。
    pub fn as_str(self) -> &'static str {
        match self {
            ColorMode::Auto => "auto",
            ColorMode::Always => "always",
            ColorMode::Never => "never",
        }
    }

    /// `--color`が受け付ける厳密な値。
    pub fn parse_exact(value: &str) -> Option<ColorMode> {
        ColorMode::ALL
            .into_iter()
            .find(|mode| mode.as_str() == value)
    }
}

impl std::fmt::Display for ColorMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
