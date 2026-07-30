//! `--color`。
//!
//! 描画条件はhelpを組み立てるより前に決まっている必要があるため、`--lang`と同じく
//! argvから副作用なく先読みする。受け付ける値と表記は[`ColorMode`]の宣言から導出し、
//! 本moduleは判定を持たない。

use std::sync::OnceLock;

use clap::Arg;
use clap::builder::PossibleValuesParser;

use crate::error::Result;
use crate::msg;
use crate::ui::ColorMode;

use super::help::Builder;

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

/// parserへ登録する`--color`。
pub fn arg(builder: &Builder) -> Result<Arg> {
    Ok(Arg::new("color")
        .long("color")
        .value_name(value_name())
        .global(true)
        .value_parser(PossibleValuesParser::new(modes()))
        // 値の一覧はFTLのhelp textに含めるため、libraryの英語固定表記は出さない。
        .hide_possible_values(true)
        .display_order(901)
        .help(builder.message(&msg!("cli-color-help", supported = mode_list()))?))
}

fn modes() -> Vec<&'static str> {
    ColorMode::ALL.iter().map(|mode| mode.as_str()).collect()
}

fn mode_list() -> String {
    modes().join(", ")
}

/// `--color`のvalue name。CLI parser libraryが`&'static str`を要求するため一度だけ組む。
fn value_name() -> &'static str {
    static VALUE_NAME: OnceLock<String> = OnceLock::new();
    VALUE_NAME.get_or_init(|| modes().join("|")).as_str()
}

#[cfg(test)]
#[path = "color_test.rs"]
mod color_test;
