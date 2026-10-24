use super::preparse_option::PreparseOption;

/// argvに指定されたlong flagがあるかを先読みする。
///
/// 値を取らないflagだけを見る。`--name=value`の形は、flagではなくoptionの綴りなので
/// 一致させない。その形が誤りであることはparserが報告する。
pub(crate) fn peek_flag(argv: &[String], option_name: PreparseOption) -> bool {
    let option_name = option_name.option_name();
    for argument in argv.iter().skip(1) {
        if argument == "--" {
            return false;
        }
        if argument.strip_prefix("--") == Some(option_name) {
            return true;
        }
    }
    false
}
