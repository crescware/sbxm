use super::edit_config;

/// 既存configの原文で、`files`の`index`番目の宣言を外す。
///
/// 宣言の直前のcommentは、その宣言のものか、続く宣言のものかを決められないため残す。
/// 最後の1件を外した`files`は`[]`になる。読み直した値が一致することは`edit_config`が
/// 確かめる。
pub(super) fn remove_file_entry(text: &str, index: usize) -> Option<String> {
    edit_config(
        text,
        |file, root| {
            let files = root.get_sequence("files")?;
            if index >= files.len() {
                return None;
            }
            files.remove(index)?;
            Some(file.to_string())
        },
        |expected| {
            if let Some(yaml_serde::Value::Sequence(files)) = expected.get_mut("files")
                && index < files.len()
            {
                files.remove(index);
            }
        },
    )
}
