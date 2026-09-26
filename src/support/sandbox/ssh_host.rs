/// `sandbox`へsshするときの宛先。
///
/// sbxが用意するProxyCommandが、この名前をSandboxへつなぐ。`sbxm open`の接続と、
/// hostのgitがSandboxのrepositoryを読み書きする経路は、同じ宛先を使う。
pub fn ssh_host(sandbox: &str) -> String {
    format!("{sandbox}.sbx")
}
