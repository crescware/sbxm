use std::sync::OnceLock;

/// color policyが受け取る三値。
///
/// 安定した受け入れ文字列、parserの候補、helpの一覧はこの型から導出する。
/// command-line adapterはargv上のoptionを見つけ、この型へ問い合わせるだけにする。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// streamがTTYのときだけ色を出す。
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

    /// policyが受け付ける安定した文字列の一覧。
    pub fn accepted_values() -> impl ExactSizeIterator<Item = &'static str> {
        Self::ALL.into_iter().map(Self::as_str)
    }

    /// policyが受け付ける値をclapのvalue nameとして表す。
    pub fn value_name() -> &'static str {
        static VALUE_NAME: OnceLock<String> = OnceLock::new();
        VALUE_NAME
            .get_or_init(|| Self::accepted_values().collect::<Vec<_>>().join("|"))
            .as_str()
    }

    /// policyが受け付ける値をhelpへ埋め込む一覧として表す。
    pub fn value_list() -> String {
        Self::accepted_values().collect::<Vec<_>>().join(", ")
    }

    /// policyが受け付ける文字列を厳密にmodeへ変換する。
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
