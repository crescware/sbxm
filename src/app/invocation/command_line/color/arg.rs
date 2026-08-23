use clap::Arg;
use clap::builder::PossibleValuesParser;

use crate::design::ColorMode;
use crate::diagnostics::Result;
use crate::msg;

use crate::app::invocation::parse::help::Builder;

/// parserへ登録する`--color`。
pub fn arg(builder: &Builder) -> Result<Arg> {
    Ok(Arg::new("color")
        .long("color")
        .value_name(ColorMode::value_name())
        .global(true)
        .value_parser(PossibleValuesParser::new(ColorMode::accepted_values()))
        // 値の一覧はFTLのhelp textに含めるため、libraryの英語固定表記は出さない。
        .hide_possible_values(true)
        .display_order(901)
        .help(builder.message(&msg!("cli-color-help", supported = ColorMode::value_list()))?))
}
