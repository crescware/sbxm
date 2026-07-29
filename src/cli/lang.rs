//! `--lang`。
//!
//! 受け付ける値も表示も組み込みlocaleの定義から導出する。言語を増やしてもこのmoduleを
//! 触らない。

use std::sync::OnceLock;

use clap::Arg;
use clap::builder::PossibleValuesParser;

use crate::error::{Error, ErrorId, Result};
use crate::i18n::Locale;
use crate::msg;

use super::help::Builder;

/// argvから先読みした`--lang`。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeekedLang {
    Absent,
    Valid(Locale),
    Invalid(String),
}

/// helpとusageを構築する前に、argvから`--lang`だけを副作用なく先読みする。
///
/// locale選択だけに使用し、ほかのargument validationやcommand実行を行わない。
pub fn peek_lang(argv: &[String]) -> PeekedLang {
    let mut arguments = argv.iter().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--" {
            break;
        }
        let value = if let Some(value) = argument.strip_prefix("--lang=") {
            Some(value.to_string())
        } else if argument == "--lang" {
            // 値が続かない場合はusage errorであり、その判定はparserへ委ねる。
            arguments.next().cloned()
        } else {
            None
        };
        if let Some(value) = value {
            return match Locale::parse_exact(&value) {
                Some(locale) => PeekedLang::Valid(locale),
                None => PeekedLang::Invalid(value),
            };
        }
    }
    PeekedLang::Absent
}

/// parserへ登録する`--lang`。
pub fn arg(builder: &Builder) -> Result<Arg> {
    Ok(Arg::new("lang")
        .long("lang")
        .value_name(value_name())
        .global(true)
        .value_parser(PossibleValuesParser::new(tags()))
        // 値の一覧はFTLのhelp textに含めるため、libraryの英語固定表記は出さない。
        .hide_possible_values(true)
        .display_order(900)
        .help(builder.message(&msg!("cli-lang-help", supported = tag_list()))?))
}

/// `--lang`の不正値に対するerror。configのvalidationより先に報告する。
pub fn invalid_lang_error(value: &str) -> Error {
    Error::new(
        ErrorId::InvalidLang,
        msg!("error-invalid-lang", value = value, supported = tag_list()),
    )
}

/// 組み込みlocaleのtag。`--lang`が受け付ける値と一致する。
fn tags() -> Vec<&'static str> {
    Locale::ALL
        .iter()
        .map(|locale| locale.as_str())
        .collect::<Vec<_>>()
}

/// helpと診断へ並べる、受け付けるlocale tagの一覧。
fn tag_list() -> String {
    tags().join(", ")
}

/// `--lang`のvalue name。CLI parser libraryが`&'static str`を要求するため一度だけ組む。
fn value_name() -> &'static str {
    static VALUE_NAME: OnceLock<String> = OnceLock::new();
    VALUE_NAME.get_or_init(|| tags().join("|")).as_str()
}

#[cfg(test)]
#[path = "lang_test.rs"]
mod lang_test;
