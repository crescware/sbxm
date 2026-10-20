/// このscopeへ1件のcustom secretが登録されている`sbx secret ls`の出力。
///
/// v0.48.0と同じJSONでscope、対象host、env、placeholderを返す。
pub fn custom_secret_listing(scope: &str, placeholder: &str) -> String {
    format!(
        r#"{{"custom_secrets":[{{"scope":"{scope}","targets":["{}"],"env":"GH_TOKEN","placeholder":"{placeholder}"}}]}}"#,
        crate::support::secret::GITHUB_HOSTS.join("\",\"")
    )
}
