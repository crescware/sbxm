//! 意味から端末styleへの唯一の写像。
//!
//! callerは`red`や`cyan`を指定しない。何を意味するかだけを[`Role`]と[`VisualState`]で
//! 示し、具体色はここだけが決める。同じ意味が全commandと全localeで同じ見え方になるのは
//! この一方向の写像があるためであり、command固有の色を作れないのもそのためである。
//!
//! 具体色はANSI標準16色のnamed colorだけを表す。固定RGBや256色indexを持たないのは
//! 機能不足ではなく、利用者のterminal themeとcontrast設定を尊重するための制約である。

use super::policy::CharacterSet;

/// 出力片が果たす役割。
///
/// `Role`は「これは何か」であり、「何色か」ではない。theme optionを将来足す場合も
/// この一覧は変えず、写像の中身だけを差し替える。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// section、guidance、diagnosticの見出し。
    Heading,
    /// tableの列名。
    TableHeader,
    /// 進行中を示すmarker。
    ProgressMarker,
    /// 成功を示すmarker。
    SuccessMarker,
    /// 注意を示すmarkerとlabel。
    WarningMarker,
    /// 失敗を示すmarkerとlabel。
    ErrorMarker,
    /// 利用者がそのままshellへ入力する一行。
    Command,
    /// 照合の基準になる短い値。
    Important,
    /// 操作説明、凡例、metadataのような補助情報。
    Muted,
    /// promptのkeyboard focusがある行。
    PromptCurrent,
    /// promptで選択済みであることを示すcheckbox。
    PromptChecked,
}

/// 状態値が文脈上どちらへ倒れているか。
///
/// 同じ文字列でも文脈で意味が変わる。`stopped`は停止commandの完了結果ならpositive、
/// 稼働要件のstatusならattentionである。値から色を推測せず、出力modelが明示する。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisualState {
    Positive,
    Attention,
    Negative,
    Neutral,
}

/// ANSI標準16色のうち、意味色として使う4色。
///
/// magentaと背景色を持たないのは、terminal themeとの衝突が大きく、意味の数に対して
/// 視覚的な負荷が高いためである。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Color {
    Red,
    Green,
    Yellow,
    Cyan,
}

/// 端末へ渡す装飾。
///
/// italic、背景色、点滅、RGB、256色indexをfieldとして持たない。禁止をcommentで守るのは
/// 守り方として弱く、表現できないほうが確実である。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct StyleSpec {
    pub bold: bool,
    /// 補助情報であることを示す減光。
    pub dim: bool,
    pub foreground: Option<Color>,
}

impl StyleSpec {
    /// 何も装飾しない。
    pub(super) fn plain() -> StyleSpec {
        StyleSpec::default()
    }

    /// 装飾を持たないか。持たないなら描画側はstyleを組み立てない。
    pub(super) fn is_plain(self) -> bool {
        self == StyleSpec::default()
    }

    fn bold() -> StyleSpec {
        StyleSpec {
            bold: true,
            ..StyleSpec::plain()
        }
    }

    fn color(foreground: Color) -> StyleSpec {
        StyleSpec {
            foreground: Some(foreground),
            ..StyleSpec::plain()
        }
    }

    fn bold_color(foreground: Color) -> StyleSpec {
        StyleSpec {
            bold: true,
            foreground: Some(foreground),
            ..StyleSpec::plain()
        }
    }
}

/// roleに対応する装飾。
pub(super) fn role_style(role: Role) -> StyleSpec {
    match role {
        Role::Heading => StyleSpec::bold(),
        // 列名は読み飛ばす対象であり、階層は示すが本文より前へ出さない。
        Role::TableHeader => StyleSpec {
            bold: true,
            dim: true,
            ..StyleSpec::plain()
        },
        Role::ProgressMarker => StyleSpec::color(Color::Cyan),
        Role::SuccessMarker => StyleSpec::color(Color::Green),
        Role::WarningMarker => StyleSpec::color(Color::Yellow),
        Role::ErrorMarker => StyleSpec::bold_color(Color::Red),
        Role::Command => StyleSpec::bold_color(Color::Cyan),
        Role::Important => StyleSpec::bold(),
        Role::Muted => StyleSpec {
            dim: true,
            ..StyleSpec::plain()
        },
        Role::PromptCurrent => StyleSpec::bold_color(Color::Cyan),
        Role::PromptChecked => StyleSpec::color(Color::Green),
    }
}

/// 状態値に対応する装飾。neutralは着色しない。
pub(super) fn state_style(state: VisualState) -> StyleSpec {
    match state {
        VisualState::Positive => StyleSpec::color(Color::Green),
        VisualState::Attention => StyleSpec::color(Color::Yellow),
        VisualState::Negative => StyleSpec::color(Color::Red),
        VisualState::Neutral => StyleSpec::plain(),
    }
}

/// markerに使う文字。
///
/// 一つの前景色で描画されるtext symbolだけを持つ。二色以上のpictographとして描画され
/// 得る文字、variation selector、ZWJ sequence、regional indicator、keycap sequenceは
/// 追加しない。現在のterminalで単色に見えることを許可理由にしない。
#[derive(Debug, Clone, Copy)]
pub(super) struct GlyphSet {
    pub progress: &'static str,
    pub success: &'static str,
    pub warning: &'static str,
    pub error: &'static str,
    pub current: &'static str,
    /// promptの操作説明が指す上下キー。key名は翻訳しない。
    pub arrow_up: &'static str,
    pub arrow_down: &'static str,
}

const UNICODE: GlyphSet = GlyphSet {
    progress: "\u{2192}",
    success: "\u{2713}",
    warning: "!",
    error: "\u{00d7}",
    current: "\u{203a}",
    arrow_up: "\u{2191}",
    arrow_down: "\u{2193}",
};

const ASCII: GlyphSet = GlyphSet {
    progress: ">",
    success: "+",
    warning: "!",
    error: "x",
    current: ">",
    arrow_up: "^",
    arrow_down: "v",
};

/// 文字集合に対応するglyph。どちらでも意味は変わらない。
pub(super) fn glyphs(characters: CharacterSet) -> GlyphSet {
    match characters {
        CharacterSet::Unicode => UNICODE,
        CharacterSet::Ascii => ASCII,
    }
}

impl GlyphSet {
    /// 定義した全glyph。testが一覧を取りこぼさないよう、宣言と同じ場所から作る。
    #[cfg(test)]
    pub(super) fn all(self) -> [&'static str; 7] {
        [
            self.progress,
            self.success,
            self.warning,
            self.error,
            self.current,
            self.arrow_up,
            self.arrow_down,
        ]
    }
}

#[cfg(test)]
#[path = "style_test.rs"]
mod style_test;
