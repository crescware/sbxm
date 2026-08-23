use clap::Arg;
use clap::builder::PossibleValuesParser;

use crate::diagnostics::Result;
use crate::i18n::Locale;
use crate::msg;

use crate::app::invocation::parse::help::Builder;

/// parserへ登録する`--lang`。
pub fn arg(builder: &Builder) -> Result<Arg> {
    Ok(Arg::new("lang")
        .long(super::OPTION_NAME)
        .value_name(Locale::value_name())
        .global(true)
        .value_parser(PossibleValuesParser::new(Locale::accepted_values()))
        // 値の一覧はFTLのhelp textに含めるため、libraryの英語固定表記は出さない。
        .hide_possible_values(true)
        .display_order(900)
        .help(builder.message(&msg!("cli-lang-help", supported = Locale::value_list()))?))
}
