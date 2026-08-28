use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::design::Fact;
use crate::diagnostics::ErrorId;

/// 拒否が`Cause:`として示した原文。
///
/// 同じerror IDで拒む道が何本もあるため、どれを通ったかはこの行でしか区別できない。
fn refusal_cause(error: &crate::diagnostics::Error) -> Checked<String> {
    error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?
        .facts
        .iter()
        .find_map(|fact| match fact {
            Fact::OneLine { label, value } if label.id == "diagnostic-cause-label" => {
                Some(value.as_str().to_string())
            }
            _ => None,
        })
        .required_because("the refusal states what could not be read")
}

/// 一覧の各entryが持つ`name`。
fn names(documents: &[serde_json::Value]) -> Vec<&str> {
    documents
        .iter()
        .filter_map(|document| document.get("name").and_then(serde_json::Value::as_str))
        .collect()
}

#[test]
fn a_listing_is_read_from_every_shape_the_command_answers_in() -> Checked {
    // 配列、1件だけの物体、1行1件。どれも同じ一覧である。形の違いで件数が変わると、
    // 呼び出し側は形ごとに数え方を持つことになる。
    for output in [
        r#"[{"name":"sbxm-a"},{"name":"sbxm-b"}]"#,
        "{\"name\":\"sbxm-a\"}\n{\"name\":\"sbxm-b\"}\n",
    ] {
        let documents = json_documents("sbx ls", output).required_because(output)?;
        assert_eq!(names(&documents), vec!["sbxm-a", "sbxm-b"]);
    }

    // 包みのない1件は、要素1つの一覧として読む。
    let documents =
        json_documents("sbx ls", r#"{"name":"sbxm-a"}"#).required_because("a lone entry")?;
    assert_eq!(names(&documents), vec!["sbxm-a"]);
    Ok(())
}

#[test]
fn an_answer_with_nothing_written_in_it_is_no_entries_rather_than_a_refusal() -> Checked {
    // 何も並んでいないことは観測できた状態である。読めなかったことと同じ扱いにすると、
    // 1件もないだけのhostがerrorとして報告される。
    for output in ["", "   \n"] {
        assert!(
            json_documents("sbx ls", output)
                .required_because("an empty answer lists nothing")?
                .is_empty()
        );
        assert!(
            wrapped_documents("sbx ls", "sandboxes", output)
                .required_because("an empty answer lists nothing")?
                .is_empty()
        );
    }
    Ok(())
}

#[test]
fn a_wrapped_listing_is_read_through_its_key_and_falls_back_without_it() -> Checked {
    let wrapped = wrapped_documents(
        "sbx ls",
        "sandboxes",
        r#"{"sandboxes":[{"name":"sbxm-a"}]}"#,
    )
    .required_because("a wrapped listing")?;
    assert_eq!(names(&wrapped), vec!["sbxm-a"]);

    // 包みの値が`null`なのは、1件もないことを示す形の1つである。
    assert!(
        wrapped_documents("sbx ls", "sandboxes", r#"{"sandboxes":null}"#)
            .required_because("a null listing")?
            .is_empty()
    );

    // 包んでいない答えは、包みなしの一覧としてそのまま読む。
    let bare = wrapped_documents("sbx ls", "sandboxes", r#"[{"name":"sbxm-a"}]"#)
        .required_because("an unwrapped listing")?;
    assert_eq!(names(&bare), vec!["sbxm-a"]);
    Ok(())
}

#[test]
fn a_document_that_is_not_a_listing_is_refused_with_the_shape_that_was_found() -> Checked {
    let error =
        json_documents("sbx ls", "12").refused_because("a number lists nothing and is not one")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    assert_eq!(
        refusal_cause(&error)?,
        "the document is neither an array nor an object"
    );

    // 包みの値が一覧でなければ、そこに何件あるかを決められない。項目名を示す。
    let error = wrapped_documents("sbx ls", "sandboxes", r#"{"sandboxes":"sbxm-a"}"#)
        .refused_because("a wrapper that holds text instead of a list")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    assert_eq!(refusal_cause(&error)?, "sandboxes is not a list");
    Ok(())
}

#[test]
fn a_line_that_is_not_json_is_refused_with_what_the_json_reader_said() -> Checked {
    // 1行1件として読み直す道でも、読めなかった位置はJSON側にしか分からない。
    let broken = "{\"name\":\"sbxm-a\"}\nnot json\n";
    let reported = serde_json::from_str::<serde_json::Value>("not json")
        .refused_because("the second line is not JSON")?
        .to_string();

    let error = json_documents("sbx ls", broken).refused_because("a stream with a broken line")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    assert_eq!(refusal_cause(&error)?, reported);
    Ok(())
}

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
