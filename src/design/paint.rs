use crate::design::style::{Color, StyleSpec};

/// 装飾を載せる。ANSIを組み立てる唯一の入口として、promptもここを通す。
pub fn paint(text: &str, spec: StyleSpec) -> String {
    if spec.is_plain() || text.is_empty() {
        return text.to_string();
    }
    console_style(spec).apply_to(text).to_string()
}

/// 意味からterminal crateのstyleへ変える唯一の場所。
///
/// `標準themeはANSI` named colorだけを使い、RGBや256色indexへ昇格しない。
fn console_style(spec: StyleSpec) -> console::Style {
    let mut style = console::Style::new().force_styling(true);
    if spec.bold {
        style = style.bold();
    }
    if spec.dim {
        style = style.dim();
    }
    if let Some(foreground) = spec.foreground {
        style = style.fg(match foreground {
            Color::Red => console::Color::Red,
            Color::Green => console::Color::Green,
            Color::Yellow => console::Color::Yellow,
            Color::Cyan => console::Color::Cyan,
        });
    }
    style
}
