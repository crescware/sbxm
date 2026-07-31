/// 1行を書く。行は必ず改行で閉じる。
pub(super) fn line(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(text.as_bytes());
    out.push(b'\n');
}
