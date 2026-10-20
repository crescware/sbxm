use crate::testing::outcome::{Checked, Required};

use super::*;

#[test]
fn a_field_that_does_not_hold_text_is_absent_rather_than_a_failure() -> Checked {
    // 型の違いをerrorにすると、読む必要のない項目まで一覧全体を止める。文字列でない値は
    // 「その項目は読めていない」として扱い、判断は呼び出し側が持つ。
    let object = serde_json::from_str::<serde_json::Value>(r#"{"name":"sbxm-a","state":7}"#)
        .required_because("the sample is JSON")?;
    let object = object
        .as_object()
        .required_because("the sample is an object")?;

    assert_eq!(string_field(object, "name"), Some("sbxm-a".to_string()));
    assert_eq!(string_field(object, "state"), None);
    assert_eq!(string_field(object, "workspace"), None);
    Ok(())
}
