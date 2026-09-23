use super::edit_config;

/// 既存configの原文で、top-levelの`entries`だけを足すか差し替える。
///
/// 既にあるkeyはその位置のまま値だけを差し替え、無いkeyは`version`の直後へ`entries`の
/// 順に足す。値の引用はyaml-editが決め、読み直した値が一致することは`edit_config`が
/// 確かめる。安全に編集できなければ`None`を返す。
pub(super) fn set_top_level(text: &str, entries: &[(&str, &str)]) -> Option<String> {
    edit_config(
        text,
        |root| {
            // 足すkeyはどれも`version`の直後へ入り、あとから足したkeyが前のkeyを押し
            // 下げる。`entries`の順に並ぶよう逆から足す。
            entries
                .iter()
                .rev()
                .all(|(key, value)| root.insert_after("version", *key, *value))
        },
        |expected| {
            for (key, value) in entries {
                expected.insert(
                    yaml_serde::Value::from(*key),
                    yaml_serde::Value::from(*value),
                );
            }
        },
    )
}
