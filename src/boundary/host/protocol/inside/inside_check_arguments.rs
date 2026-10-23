use super::INSIDE_CHECK;

/// `INSIDE_CHECK`へ`commands`を渡す`sbx exec`の内側のargv。
///
/// `sh -c <手順> sh <argv>... "" <argv>... ""`の形であり、各commandの後ろに区切りの
/// 空の引数を置く。commandのargvに空の引数は無いものとする。
pub fn inside_check_arguments(commands: &[Vec<String>]) -> Vec<String> {
    let mut args = vec![
        "sh".to_string(),
        "-c".to_string(),
        INSIDE_CHECK.to_string(),
        "sh".to_string(),
    ];
    for command in commands {
        args.extend(command.iter().cloned());
        args.push(String::new());
    }
    args
}
