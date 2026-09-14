/// このscopeへ`github` serviceのtokenが登録されている`sbx secret ls --json`の出力。
///
/// 実機と同じく、値は`(stored)`と伏せて返る。
pub fn service_secret_listing(scope: &str) -> String {
    format!(
        r#"{{"secrets":[{{"scope":"{scope}","type":"service","name":"github","secret":"(stored)"}}],"custom_secrets":[],"shadowed_services":[],"env_only_count":0}}"#
    )
}
