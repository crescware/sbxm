use super::{GITHUB_HOSTS, GITHUB_TOKEN_ENV};

/// tokenを登録するcommand。
///
/// `add`の案内と、未登録で停止したときの是正指示で同じ文字列を使う。
///
/// `--host`は`stringArray`であり、繰り返して渡す。区切り文字1つで並べた値は1件のhost
/// 名として読まれる。wildcardはshellに食われるため引用符で囲む。
///
/// 同じenvのcustom secretが既にある場合、`set-custom`はそれを重複として拒否する。
/// 既存のplaceholderを`--placeholder`で明示すると更新として通り、しかもSandboxが
/// 持つ値が変わらないため、作り直さずに済む。
pub fn register_command(sandbox: &str, placeholder: Option<&str>) -> String {
    let hosts = GITHUB_HOSTS
        .iter()
        .map(|host| format!("--host '{host}'"))
        .collect::<Vec<String>>()
        .join(" ");
    let keep = match placeholder {
        Some(placeholder) => format!(" --placeholder {placeholder}"),
        None => String::new(),
    };
    format!(
        "sbx secret set-custom {sandbox} {hosts}{keep} --env {GITHUB_TOKEN_ENV} --value <token>"
    )
}
