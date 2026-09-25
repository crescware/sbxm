use super::*;

#[test]
fn known_inputs_produce_the_published_digest() {
    assert_eq!(
        sha256_hex(b"owner/repository"),
        "21da1b566c9f5080441b32f57a37f33e921c5d8be8b686584f9ce819d66732fe"
    );
    assert_eq!(
        sha256_hex(b"example-org/example-repo"),
        "99a40327a69bb5dc074f625c045448b0f3fe0587155e74276758abce34759ead"
    );
}

#[test]
fn digests_are_lowercase_hex_of_a_fixed_length() {
    let digest = sha256_hex(b"a/b");
    assert_eq!(digest.len(), 64);
    assert!(
        digest
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    );
}

#[test]
fn the_short_form_keeps_the_leading_twelve_digits() {
    let full = sha256_hex(b"a/b");
    assert_eq!(short_hex(&full), "c14cddc033f6");
    assert_eq!(short_hex(&full).len(), SHORT_HEX_LENGTH);
    assert_eq!(short_hex("abc"), "abc");
}

#[test]
fn a_file_digest_matches_the_digest_of_its_bytes() -> crate::testing::outcome::Checked {
    use crate::testing::outcome::{Refused, Required};

    let dir = tempfile::tempdir().required()?;
    let path = dir.path().join("large.bundle");
    let bytes: Vec<u8> = (0..=255u8).cycle().take(200_000).collect();
    std::fs::write(&path, &bytes).required()?;
    assert_eq!(
        super::sha256_file_hex(&path).required()?,
        super::sha256_hex(&bytes)
    );
    super::sha256_file_hex(&dir.path().join("missing")).refused_because("no such file")?;
    Ok(())
}
