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

#[test]
fn the_image_inspect_parser_reads_the_identity_and_the_labels() -> Checked {
    let output = r#"[{"Id":"sha256:abc","Config":{"Labels":{"io.crescware.sbxm.canonical-id":"example-org/example-repo"}}}]"#;
    let identity = parse_image_inspect(output).required_because("a single image parses")?;
    assert_eq!(identity.id, "sha256:abc");
    assert_eq!(
        identity
            .labels
            .get("io.crescware.sbxm.canonical-id")
            .map(String::as_str),
        Some("example-org/example-repo")
    );

    // labelを持たないimageは、labelが空のimageとして読む。
    let identity = parse_image_inspect(r#"[{"Id":"sha256:abc","Config":{"Labels":null}}]"#)
        .required_because("an image without labels parses")?;
    assert!(identity.labels.is_empty());
    Ok(())
}

#[test]
fn an_output_that_is_not_json_is_refused_with_what_the_json_reader_said() -> Checked {
    // 読めなかった位置はJSON側にしか分からない。sbxmが言い換えると、原文のどこで
    // 途切れたかが読み手から消える。
    let broken = r#"[{"Id":"sha256:a","Config":{}}"#;
    let reported = serde_json::from_str::<serde_json::Value>(broken)
        .refused_because("the sample is not JSON")?
        .to_string();

    let error = parse_image_inspect(broken).refused_because("a truncated document")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    assert_eq!(refusal_cause(&error)?, reported);
    Ok(())
}

#[test]
fn an_image_inspect_output_that_cannot_be_read_states_which_reading_failed() -> Checked {
    // labelはimageの同一性を照合する材料である。読めない形をlabelなしへ丸めると、
    // 別物のimageを同じものとして扱う。何が読めなかったかを分けて示す。
    for (output, cause) in [
        // 配列の要素が物体でなければ、IdもLabelsも読む場所がない。
        (r#"["sha256:a"]"#, "the entry is not an object"),
        // Labelsは物体か`null`のどちらかで、一覧ではない。
        (
            r#"[{"Id":"sha256:a","Config":{"Labels":["io.crescware.sbxm.canonical-id"]}}]"#,
            "Labels is neither an object nor null",
        ),
        // 値が文字列でないlabelは、どのlabelかを示さないと出力から探せない。
        (
            r#"[{"Id":"sha256:a","Config":{"Labels":{"io.crescware.sbxm.canonical-id":1}}}]"#,
            "label io.crescware.sbxm.canonical-id does not hold a string",
        ),
        (r#"[{"Id":"sha256:a"}]"#, "the image has no Config section"),
        (r#"[{"Id":"","Config":{}}]"#, "the image has no Id"),
        (r#"[{"Config":{}}]"#, "the image has no Id"),
        ("[]", "the document describes 0 images instead of one"),
        (
            r#"[{"Id":"sha256:a","Config":{}},{"Id":"sha256:b","Config":{}}]"#,
            "the document describes 2 images instead of one",
        ),
        (r#"{"Id":"sha256:a"}"#, "the document is not an array"),
    ] {
        let error = parse_image_inspect(output).refused_because("an image that cannot be read")?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
        assert_eq!(refusal_cause(&error)?, cause, "output {output}");
    }
    Ok(())
}
