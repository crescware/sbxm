use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::error::ErrorId;

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
fn an_image_inspect_output_that_is_not_one_image_is_refused() -> Checked {
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
        let error = parse_image_inspect(output).refused_because("{output} must be refused")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::ExternalOutputUnparseable),
            "output {output} produced the wrong error"
        );
    }
    Ok(())
}
