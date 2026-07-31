/// custom secretが1件も登録されていないscopeの出力。
pub fn no_custom_secrets(scope: &str) -> String {
    format!("No secrets found for scope \"{scope}\".\n")
}
