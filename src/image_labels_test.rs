use crate::testing::outcome::{Checked, Required};

use super::*;

#[test]
fn an_image_that_declares_no_labels_has_an_empty_label_map() {
    assert_eq!(labels_from_declared(None), Ok(BTreeMap::new()));
    assert_eq!(
        labels_from_declared(Some(&serde_json::Value::Null)),
        Ok(BTreeMap::new())
    );
    assert_eq!(
        labels_from_declared(Some(&serde_json::json!({}))),
        Ok(BTreeMap::new())
    );
}

#[test]
fn declared_labels_are_read_as_strings_keyed_by_name() -> Checked {
    let declared = serde_json::json!({"b": "second", "a": "first"});
    let labels = labels_from_declared(Some(&declared)).required()?;
    assert_eq!(
        labels.into_iter().collect::<Vec<_>>(),
        vec![
            ("a".to_string(), "first".to_string()),
            ("b".to_string(), "second".to_string()),
        ]
    );
    Ok(())
}

#[test]
fn a_declaration_that_is_not_an_object_is_not_read_as_a_label_map() {
    assert_eq!(
        labels_from_declared(Some(&serde_json::json!("io.crescware.sbxm.canonical-id"))),
        Err(LabelDefect::NotAnObject)
    );
    assert_eq!(
        labels_from_declared(Some(&serde_json::json!([]))),
        Err(LabelDefect::NotAnObject)
    );
}

#[test]
fn a_label_whose_value_is_not_a_string_names_its_key() {
    assert_eq!(
        labels_from_declared(Some(&serde_json::json!({"a": 1}))),
        Err(LabelDefect::ValueNotAString("a".to_string()))
    );
}
