/// 以前の版が案内した`GH_TOKEN`のcustom secretだけが登録されている`sbx secret ls --json`の出力。
///
/// service secretは無い。組み込み`github` serviceがこのenvを予約するため、この登録は
/// Sandboxへ届かない。
pub fn custom_secret_listing(scope: &str, placeholder: &str) -> String {
    format!(
        r#"{{"secrets":[],"custom_secrets":[{{"scope":"{scope}","targets":["github.com","**.github.com","**.githubusercontent.com","ghcr.io"],"env":"GH_TOKEN","placeholder":"{placeholder}","secret":"github******...******6KGT"}}],"shadowed_services":[],"env_only_count":0}}"#
    )
}
