use yaml_edit::{Mapping, YamlFile};

/// 既存configの原文を、構文木の上で編集する。
///
/// 利用者が手で書いたコメント、空行、key順、引用の仕方は、編集した箇所の外ではそのまま
/// 残る。`edit`はtop-levelのmappingを書き換え、書き換えられなかったときは`false`を返す。
/// `expect`は同じ変更を、原文から読んだ値へ当てる。
///
/// 次のいずれかに当たるときは`None`を返し、呼び出し側が利用者のfileを書き換えずに拒否する。
///
/// - 原文に構文errorがある。yaml-editはerrorがあっても解析を続け、壊れた部分を含む木を返す
/// - 解析してそのまま描き戻した結果が原文と一致しない。編集していない箇所まで変わりうる
/// - documentが1つでない、またはtop-levelがmappingでない
/// - 編集結果を読み直した値が、原文の値へ同じ変更を当てた値と一致しない
pub(super) fn edit_config(
    text: &str,
    edit: impl FnOnce(&Mapping) -> bool,
    expect: impl FnOnce(&mut yaml_serde::Mapping),
) -> Option<String> {
    let parsed = YamlFile::parse(text);
    if parsed.has_errors() {
        return None;
    }
    let file = parsed.tree();
    if file.to_string() != text {
        return None;
    }
    let mut documents = file.documents();
    let (Some(document), None) = (documents.next(), documents.next()) else {
        return None;
    };
    let root = document.as_mapping()?;
    let Ok(yaml_serde::Value::Mapping(mut expected)) = yaml_serde::from_str(text) else {
        return None;
    };

    if !edit(&root) {
        return None;
    }
    expect(&mut expected);

    // 木の上の編集は、結果の値までは保証しない。anchorを外して別のkeyのaliasを宙に
    // 浮かせることも、隣の値を壊すこともありうる。変えたkeyだけでなく文書全体を読み直し、
    // 変えていない値がすべて元のままであることを確かめる。
    let updated = file.to_string();
    let actual: yaml_serde::Value = yaml_serde::from_str(&updated).ok()?;
    (actual == yaml_serde::Value::Mapping(expected)).then_some(updated)
}
