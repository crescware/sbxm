use super::INSIDE_CHECK;

/// `sbx`の引数がまとめた確認なら、Sandbox名と、所要時間を計るかと、並べられたcommandの一覧。
///
/// 引数は`exec <name> -- sh -c <手順> sh [-t] <argv>... "" ...`の形であり、空の引数が
/// commandの区切りである。`inside_check_arguments`が作る形の逆を読む。
pub fn split_inside_check(args: &[String]) -> Option<(String, bool, Vec<Vec<String>>)> {
    let position = args.iter().position(|arg| arg == "--")?;
    let sandbox = args.get(position.checked_sub(1)?)?.clone();
    let [script_shell, option, script, inner_shell, rest @ ..] = &args[position + 1..] else {
        return None;
    };
    if script_shell != "sh" || option != "-c" || script != INSIDE_CHECK || inner_shell != "sh" {
        return None;
    }
    let (timed, rest) = match rest {
        [first, rest @ ..] if first == "-t" => (true, rest),
        rest => (false, rest),
    };
    let mut commands = Vec::new();
    let mut current = Vec::new();
    for arg in rest {
        if arg.is_empty() {
            commands.push(std::mem::take(&mut current));
        } else {
            current.push(arg.clone());
        }
    }
    Some((sandbox, timed, commands))
}
