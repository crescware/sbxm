use super::HELPER_PREFIX;

/// Sandbox内のgitへ設定する、期待するcredential helperの値。
///
/// placeholderをそのままpasswordとして返す。gitはこれをBasic認証として送り、proxyが
/// 登録済みhost宛のrequestで本物のtokenへ差し替える。placeholderはtokenではなく、
/// `sbx secret ls`が誰にでも示す公開の目印であり、Sandboxへ置いても秘密は漏れない。
///
/// Sandboxの環境変数を経由しないのは、Docker Sandboxesが`GH_TOKEN`のような名前を
/// 組み込みserviceのために予約し、同名のcustom secretのplaceholderをSandboxへ
/// 届けないためである。requestの中にplaceholderが現れさえすればproxyは差し替えるので、
/// 環境変数に依存する理由がない。
pub(super) fn expected_credential_helper(placeholder: &str) -> String {
    format!("{HELPER_PREFIX}{placeholder}; }}; f")
}
