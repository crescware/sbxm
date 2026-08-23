use clap::Arg;
use clap::builder::PossibleValuesParser;

use crate::diagnostics::Result;
use crate::msg;

use crate::app::invocation::parse::help::Builder;

use super::{tag_list, tags, value_name};

/// parserへ登録する`--lang`。
pub fn arg(builder: &Builder) -> Result<Arg> {
    Ok(Arg::new("lang")
        .long(super::LONG)
        .value_name(value_name())
        .global(true)
        .value_parser(PossibleValuesParser::new(tags()))
        // 値の一覧はFTLのhelp textに含めるため、libraryの英語固定表記は出さない。
        .hide_possible_values(true)
        .display_order(900)
        .help(builder.message(&msg!("cli-lang-help", supported = tag_list()))?))
}
