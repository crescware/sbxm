/// `ssh -G <宛先>`が示した実効設定に、ProxyCommandがあるか。
///
/// sbxが用意するProxyCommandが、`<sandbox>.sbx`をSandboxへつなぐ。構築の前の確認と
/// `status --global`のRemote SSHは、同じこの判定を使う。
pub fn proxy_command_configured(effective: &str) -> bool {
    effective
        .lines()
        .any(|line| line.trim().to_ascii_lowercase().starts_with("proxycommand"))
}
