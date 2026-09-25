/// `sbx exec`へ渡す引数。
///
/// `stdin`が真なら`-i`を付け、hostのstdinを中のcommandへつなぐ。`sbx exec`は`-i`が
/// 無ければstdinを中へつながない。
pub(super) fn exec_arguments(
    sandbox: &str,
    user: Option<&str>,
    stdin: bool,
    args: &[&str],
) -> Vec<String> {
    let mut full: Vec<String> = vec!["exec".to_string()];
    if stdin {
        full.push("-i".to_string());
    }
    if let Some(user) = user {
        full.push("--user".to_string());
        full.push(user.to_string());
    }
    full.push(sandbox.to_string());
    full.push("--".to_string());
    full.extend(args.iter().map(|arg| (*arg).to_string()));
    full
}
