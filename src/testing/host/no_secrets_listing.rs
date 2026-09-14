/// 1件も登録がない`sbx secret ls --json`の出力。
pub fn no_secrets_listing() -> String {
    r#"{"secrets":[],"custom_secrets":[],"shadowed_services":[],"env_only_count":0}"#.to_string()
}
