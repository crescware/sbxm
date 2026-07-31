/// SHA-256のlowercase hexであること。
pub fn require_sha256(value: &str) -> std::result::Result<(), String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(format!("{value} is not a lowercase SHA-256 hex digest"));
    }
    Ok(())
}
