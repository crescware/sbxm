use crate::diagnostics::Msg;
use crate::msg;

/// SHA-256のlowercase hexであること。
///
/// 受け付けられない理由は、報告する側が翻訳できるようmessageで返す。
pub fn require_sha256(value: &str) -> std::result::Result<(), Msg> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(msg!("cause-not-a-sha256", observed = value));
    }
    Ok(())
}
