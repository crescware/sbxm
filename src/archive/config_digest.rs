/// `blobs/sha256/<hex>`と`<hex>.json`のどちらの書き方でも、`sha256:<hex>`へ寄せる。
pub(super) fn config_digest(config: &str) -> Option<String> {
    let name = config.rsplit('/').next()?;
    let hex = name.strip_suffix(".json").unwrap_or(name);
    let hex = hex.strip_prefix("sha256:").unwrap_or(hex);
    if hex.len() != 64 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some(format!("sha256:{}", hex.to_ascii_lowercase()))
}
