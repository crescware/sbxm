/// このscopeへ1件のcustom secretが登録されている`sbx secret ls`の出力。
///
/// 実機と同じく、scope名で始まる行に対象host、env、placeholderが続く。
pub fn custom_secret_listing(scope: &str, placeholder: &str) -> String {
    format!(
        "CUSTOM SECRETS\nSCOPE   TARGETS   ENV   PLACEHOLDER   SECRET\n{scope}   {}   GH_TOKEN   {placeholder}   ghp_example\n",
        crate::support::secret::GITHUB_HOSTS.join(" ")
    )
}
