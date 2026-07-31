use crate::i18n::Locale;

use super::PeekedLang;

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
