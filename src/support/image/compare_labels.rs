use crate::compatibility::ImageIdentity;

/// 期待するlabelと観測したlabelの並び。翻訳しない技術表記。
///
/// 1 labelを1行とする。事実の値が複数行になると、rendererが項目名の下へ字下げして
/// 並べるため、labelごとの差分をそのまま読める。
pub(super) fn compare_labels(identity: &ImageIdentity, expected: &[(String, String)]) -> String {
    expected
        .iter()
        .map(|(key, value)| {
            let observed = identity.labels.get(key).map_or("<absent>", String::as_str);
            format!("{key}: expected {value}, observed {observed}")
        })
        .collect::<Vec<_>>()
        .join("\n")
}
