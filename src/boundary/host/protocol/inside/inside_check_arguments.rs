use super::INSIDE_CHECK;

/// `INSIDE_CHECK`へ`commands`を渡す`sbx exec`の内側のargv。
///
/// `sh -c <手順> sh [-t] <argv>... "" <argv>... ""`の形であり、各commandの後ろに区切りの
/// 空の引数を置く。commandのargvに空の引数は無いものとする。`timed`なら`-t`を先頭に
/// 置き、手順にcommandごとの所要時間を計らせる。
pub fn inside_check_arguments(commands: &[Vec<String>], timed: bool) -> Vec<String> {
    let mut args = vec![
        "sh".to_string(),
        "-c".to_string(),
        INSIDE_CHECK.to_string(),
        "sh".to_string(),
    ];
    if timed {
        args.push("-t".to_string());
    }
    for command in commands {
        args.extend(command.iter().cloned());
        args.push(String::new());
    }
    args
}
