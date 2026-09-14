/// image configのdigestから、同じimageのindex digestとして使う別の値を導く。
///
/// 実物ではindexのdigestはconfigのdigestと無関係だが、testでは2つが違う値であり、
/// かつconfigから決まることだけが要る。16進の各桁を1つずらす。
pub fn index_id(image_id: &str) -> String {
    let hex = image_id.strip_prefix("sha256:").unwrap_or(image_id);
    let shifted: String = hex
        .chars()
        .map(|digit| {
            let value = digit.to_digit(16).unwrap_or(0);
            char::from_digit((value + 1) % 16, 16).unwrap_or('0')
        })
        .collect();
    format!("sha256:{shifted}")
}
