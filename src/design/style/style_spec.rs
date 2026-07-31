use super::Color;

/// 端末へ渡す装飾。
///
/// italic、背景色、点滅、RGB、256色indexをfieldとして持たない。禁止をcommentで守るのは
/// 守り方として弱く、表現できないほうが確実である。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StyleSpec {
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
    pub(crate) fn is_plain(self) -> bool {
        self == StyleSpec::default()
    }

    pub(crate) fn bold() -> StyleSpec {
        StyleSpec {
            bold: true,
            ..StyleSpec::plain()
        }
    }

    pub(crate) fn color(foreground: Color) -> StyleSpec {
        StyleSpec {
            foreground: Some(foreground),
            ..StyleSpec::plain()
        }
    }

    pub(crate) fn bold_color(foreground: Color) -> StyleSpec {
        StyleSpec {
            bold: true,
            foreground: Some(foreground),
            ..StyleSpec::plain()
        }
    }
}
