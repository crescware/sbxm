use super::{GITHUB_HOSTS, GITHUB_TOKEN_ENV, is_global_scope};

/// tokenを登録するcommand。
///
/// `add`の案内と、未登録で停止したときの是正指示で同じ文字列を使う。
///
/// `--host`は`stringArray`であり、繰り返して渡す。区切り文字1つで並べた値は1件のhost
/// 名として読まれる。wildcardはshellに食われるため引用符で囲む。
///
/// `--env`は、Sandboxの中でtokenを読む利用者のために残す。sbxm自身のgitは環境変数を
/// 使わず、placeholderをcredential helperへ直接持たせるため、この変数が届かなくても
/// clone、fetch、pushは通る。
///
/// `scope`がglobalなら、scope引数を渡さない。globalは`set-custom`の既定であり、Sandbox名を
/// 渡すと同じplaceholderでも別scopeの登録を作ろうとしてしまう。
///
/// 同じenvのcustom secretが既にある場合、`set-custom`はそれを重複として拒否する。
/// 既存のplaceholderを`--placeholder`で明示すると更新として通り、しかもSandboxが
/// 持つ値が変わらないため、作り直さずに済む。
pub fn register_command(scope: &str, placeholder: Option<&str>) -> String {
    let hosts = GITHUB_HOSTS
        .iter()
        .map(|host| format!("--host '{host}'"))
        .collect::<Vec<String>>()
        .join(" ");
    let keep = match placeholder {
        Some(placeholder) => format!(" --placeholder {placeholder}"),
        None => String::new(),
    };
    let scope = if is_global_scope(scope) {
        String::new()
    } else {
        format!("{scope} ")
    };
    format!("sbx secret set-custom {scope}{hosts}{keep} --env {GITHUB_TOKEN_ENV} --value <token>")
}
