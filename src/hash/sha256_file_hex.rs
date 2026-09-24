use std::fmt::Write as _;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

use sha2::{Digest, Sha256};

/// fileの中身のSHA-256のlowercase hex。
///
/// bundleのように大きなfileを、memoryへ読み込まずに少しずつ数える。
pub fn sha256_file_hex(path: &Path) -> io::Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let mut out = String::with_capacity(64);
    for byte in hasher.finalize() {
        // Stringへの書き込みは失敗しない。
        let _ = write!(out, "{byte:02x}");
    }
    Ok(out)
}
