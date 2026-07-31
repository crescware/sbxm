use clap::Arg;
use clap::builder::PossibleValuesParser;

use crate::diagnostics::Result;
use crate::msg;

use crate::cli::help::Builder;

use super::{modes, value_name};

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

fn mode_list() -> String {
    modes().join(", ")
}
