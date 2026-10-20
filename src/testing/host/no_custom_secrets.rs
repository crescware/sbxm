/// custom secretが1件も登録されていないscopeの出力。
pub fn no_custom_secrets(_scope: &str) -> String {
    r#"{"custom_secrets":[]}"#.to_string()
}
