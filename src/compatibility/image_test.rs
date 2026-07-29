use super::*;
use crate::error::ErrorId;

#[test]
fn the_image_inspect_parser_reads_the_identity_and_the_labels() {
    let output = r#"[{"Id":"sha256:abc","Config":{"Labels":{"io.crescware.sbxm.canonical-id":"example-org/example-repo"}}}]"#;
    let identity = parse_image_inspect(output).expect("a single image parses");
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
        .expect("an image without labels parses");
    assert!(identity.labels.is_empty());
}

#[test]
fn an_image_inspect_output_that_is_not_one_image_is_refused() {
    for output in [
        "[]",
        r#"[{"Id":"sha256:a","Config":{}},{"Id":"sha256:b","Config":{}}]"#,
        r#"[{"Config":{}}]"#,
        r#"[{"Id":"","Config":{}}]"#,
        r#"[{"Id":"sha256:a"}]"#,
        r#"{"Id":"sha256:a"}"#,
        r#"[{"Id":"sha256:a","Config":{"Labels":{"key":1}}}]"#,
        "not json",
    ] {
        let error = parse_image_inspect(output).expect_err("{output} must be refused");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::ExternalOutputUnparseable),
            "output {output} produced the wrong error"
        );
    }
}
