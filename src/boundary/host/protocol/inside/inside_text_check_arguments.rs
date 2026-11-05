use super::inside_check_arguments;

/// NULを含まないtextの検査。stdoutとstderrをまとめ、成功時はstdout、失敗時はstderrへ返す。
/// 一時fileを作らず、末尾の改行も保持する。
pub fn inside_text_check_arguments(commands: &[Vec<String>]) -> Vec<String> {
    let mut args = inside_check_arguments(commands, false);
    args.insert(4, "-m".to_string());
    args
}
