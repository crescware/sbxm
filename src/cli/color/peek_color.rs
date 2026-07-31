use crate::design::ColorMode;

/// helpとusageを構築する前に、argvから`--color`だけを先読みする。
///
/// 値が不正な場合もここでは失敗させない。優先順位の1番目である明示指定が無いものとして
/// 扱い、判定はほかのargument validationと同じ順序でparserへ委ねる。
pub fn peek_color(argv: &[String]) -> ColorMode {
    let mut arguments = argv.iter().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--" {
            break;
        }
        let value = if let Some(value) = argument.strip_prefix("--color=") {
            Some(value.to_string())
        } else if argument == "--color" {
            arguments.next().cloned()
        } else {
            None
        };
        if let Some(value) = value {
            return ColorMode::parse_exact(&value).unwrap_or_default();
        }
    }
    ColorMode::default()
}
