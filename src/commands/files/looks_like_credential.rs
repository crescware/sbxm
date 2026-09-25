/// file名が、認証情報を持つfileによく使われる名前か。
///
/// 名前だけで中身は判断できない。拒否はせず、宣言した本人へ確かめさせるための目安である。
pub fn looks_like_credential(name: &str) -> bool {
    let name = name.to_ascii_lowercase();
    let exact = [".env", ".netrc", ".pgpass", ".git-credentials"];
    let contained = [
        "credential",
        "secret",
        "token",
        "password",
        "id_rsa",
        "id_dsa",
        "id_ecdsa",
        "id_ed25519",
    ];
    let suffixes = [".pem", ".key", ".p12", ".pfx"];
    exact.contains(&name.as_str())
        || name.starts_with(".env.")
        || contained.iter().any(|part| name.contains(part))
        || suffixes.iter().any(|suffix| name.ends_with(suffix))
}
