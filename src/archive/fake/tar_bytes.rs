use crate::archive::BLOCK;

/// 検証したい形のarchiveを組み立てる、最小限のtar writer。
///
/// 外部commandが書くarchiveを、testの中で再現するために使う。
pub fn tar_bytes(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    for (name, data) in entries {
        let mut header = [0_u8; BLOCK];
        let name_bytes = name.as_bytes();
        header[..name_bytes.len()].copy_from_slice(name_bytes);
        let size = format!("{:011o}\0", data.len());
        header[124..124 + size.len()].copy_from_slice(size.as_bytes());
        header[156] = b'0';
        header[257..262].copy_from_slice(b"ustar");
        out.extend_from_slice(&header);
        out.extend_from_slice(data);
        let padding = (BLOCK - data.len() % BLOCK) % BLOCK;
        out.extend(std::iter::repeat_n(0_u8, padding));
    }
    out.extend(std::iter::repeat_n(0_u8, BLOCK * 2));
    out
}
