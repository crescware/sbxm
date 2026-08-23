use clap::Arg;
use clap::builder::PossibleValuesParser;

use crate::design::ColorMode;
use crate::diagnostics::Result;
use crate::msg;

use crate::app::invocation::parse::help::Builder;

use super::PreparseOption;

/// parserへ登録する`--color`。
pub(super) fn color_arg(builder: &Builder) -> Result<Arg> {
    Ok(PreparseOption::Color
        .arg()
        .value_name(ColorMode::value_name())
        .value_parser(PossibleValuesParser::new(ColorMode::accepted_values()))
        .hide_possible_values(true)
        .display_order(901)
        .help(builder.message(&msg!("cli-color-help", supported = ColorMode::value_list()))?))
}
