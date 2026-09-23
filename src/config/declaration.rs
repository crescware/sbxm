use crate::diagnostics::Result;

use super::serialized;

/// 利用者へ手で書き足してもらう名義の1行を、YAML自身の引用規則で組み立てる。
///
/// 名前もmail addressも利用者が打った任意の文字列であり、`#`や`:`のようにYAMLの
/// 意味を持つ文字を含みうる。そのまま貼れば同じ値として読めるよう、引用もserializerへ
/// 決めさせる。
///
/// `validate_git_identity_value`が改行を拒むため、値は必ず1行に収まる。1 keyの
/// mappingは1行として描かれる。
pub(super) fn declaration(key: &str, value: &str) -> Result<String> {
    let mut mapping = yaml_serde::Mapping::new();
    mapping.insert(
        yaml_serde::Value::String(key.to_string()),
        yaml_serde::Value::String(value.to_string()),
    );
    Ok(serialized(&mapping, "config.yaml")?.trim_end().to_string())
}
