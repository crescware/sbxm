use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::diagnostics::ErrorId;

#[test]
fn the_template_listing_of_the_target_version_is_read_as_it_is() -> Checked {
    // 対象versionが実際に出力する形。`images`で包み、1件をrepositoryとtagで示す。
    let observed = r#"{
  "images": [
    {
      "id": "a3d0f4449170",
      "repository": "docker.io/library/sbxm-example-org-example-repo-0123456789ab-template",
      "tag": "548a91cfab02",
      "flavor": "shell-docker",
      "created_at": "2026-07-27T03:12:26Z",
      "size": 841254707
    }
  ]
}"#;

    let entries = parse_template_list(observed).required_because("the real listing parses")?;
    assert_eq!(entries.len(), 1);

    // sbxmが渡す名前はregistry prefixを持たない。runtimeは補って表示する。
    assert!(
        entries[0].is_named("sbxm-example-org-example-repo-0123456789ab-template:548a91cfab02")
    );
    assert!(entries[0].is_named(
        "docker.io/library/sbxm-example-org-example-repo-0123456789ab-template:548a91cfab02"
    ));
    assert!(!entries[0].is_named("sbxm-example-org-example-repo-0123456789ab-template:other"));

    assert!(
        parse_template_list(r#"{"images": []}"#)
            .required()?
            .is_empty()
    );
    assert!(parse_template_list("").required()?.is_empty());

    // repositoryとtagのどちらかを欠く一覧からは、対応を決められない。
    for output in [
        r#"{"images":[{"id":"a3d0f4449170","tag":"v1"}]}"#,
        r#"{"images":[{"id":"a3d0f4449170","repository":"docker.io/library/x"}]}"#,
        r#"{"images":[{"repository":"","tag":"v1"}]}"#,
        "12",
    ] {
        let error = parse_template_list(output).refused_because("{output} must be refused")?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    }
    Ok(())
}
