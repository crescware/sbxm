use super::preparse_option::PreparseOption;

/// argvから指定されたlong optionの生値だけを先読みする。
///
/// optionの意味や値のvalidationは呼び出し側に委ねる。値が不正でも、先読みの順序を
/// parserより先に変えないために、ここでは単に文字列を返す。
pub(crate) fn peek(argv: &[String], option_name: PreparseOption) -> Option<&str> {
    let option_name = option_name.option_name();
    let mut arguments = argv.iter().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--" {
            break;
        }
        let Some(argument) = argument.strip_prefix("--") else {
            continue;
        };
        if argument == option_name {
            return arguments.next().map(String::as_str);
        }
        if let Some(value) = argument
            .strip_prefix(option_name)
            .and_then(|suffix| suffix.strip_prefix('='))
        {
            return Some(value);
        }
    }
    None
}
