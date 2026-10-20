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
fn the_template_listing_of_the_target_version_is_read_as_it_is() -> Checked {
    // 対象versionが実際に出力する形。配列で返し、1件をrepositoryとtagで示す。
    let observed = r#"[
    {
      "id": "a3d0f4449170",
      "repository": "docker.io/library/sbxm-example-org-example-repo-0123456789ab-template",
      "tag": "548a91cfab02",
      "flavor": "shell-docker",
      "created_at": "2026-07-27T03:12:26Z",
      "size": 841254707
    }
 ]"#;

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

    assert!(parse_template_list("[]").required()?.is_empty());
    Ok(())
}

#[test]
fn a_legacy_or_partial_listing_is_not_read_as_an_empty_array() -> Checked {
    // v0.48.0は1件もなくても`[]`を返す。旧版の包み、`null`、空の出力、1行1件のJSONを
    // 一覧として読むと、読めなかった一覧を「Templateがない」と取り違える。
    for (output, cause) in [
        ("", "EOF while parsing a value at line 1 column 0"),
        ("null", "the document is not an array"),
        (r#"{"images":[]}"#, "the document is not an array"),
        (r#"{"images":null}"#, "the document is not an array"),
        (
            r#"{"repository":"docker.io/library/x","tag":"v1"}"#,
            "the document is not an array",
        ),
        (
            "{\"repository\":\"x\",\"tag\":\"v1\"}\n{\"repository\":\"y\",\"tag\":\"v1\"}\n",
            "trailing characters at line 2 column 1",
        ),
    ] {
        let error = parse_template_list(output).refused_because(output)?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
        assert_eq!(refusal_cause(&error)?, cause, "output {output}");
    }
    Ok(())
}

#[test]
fn only_the_field_names_of_the_target_version_are_read() -> Checked {
    // 大文字の項目名を返す実versionの根拠はない。別名から名前を拾わない。
    let error = parse_template_list(r#"[{"Repository":"docker.io/library/x","Tag":"v1"}]"#)
        .refused_because("capitalized fields are not an alias")?;
    assert_eq!(refusal_cause(&error)?, "an entry has no repository");

    let entries =
        parse_template_list(r#"[{"Id":"a3d0f4449170","repository":"x","tag":"v1"}]"#).required()?;
    assert_eq!(entries[0].id, None);
    Ok(())
}

#[test]
fn a_repository_keeps_only_the_names_that_actually_point_at_it() -> Checked {
    // registry prefixを剥がした名前は、剥がせたときだけ足す。持っていない名前を
    // 補うと、別のregistryのimageを同じTemplateとして選ぶ。
    for (repository, names) in [
        (
            "sbxm-example-template",
            vec!["sbxm-example-template:v1".to_string()],
        ),
        (
            "docker.io/crescware/sbxm-example-template",
            vec![
                "docker.io/crescware/sbxm-example-template:v1".to_string(),
                "crescware/sbxm-example-template:v1".to_string(),
            ],
        ),
        (
            "docker.io/library/sbxm-example-template",
            vec![
                "docker.io/library/sbxm-example-template:v1".to_string(),
                "sbxm-example-template:v1".to_string(),
            ],
        ),
    ] {
        let output = format!(r#"[{{"repository":"{repository}","tag":"v1"}}]"#);
        let entries = parse_template_list(&output).required_because(&output)?;
        assert_eq!(entries[0].names, names);
        // 剥がした表記の1つだけを持つ側から照合しても、同じentryへ届く。
        assert!(entries[0].is_named(&format!("{repository}:v1")));
    }
    Ok(())
}

#[test]
fn a_template_listing_that_cannot_be_read_states_which_reading_failed() -> Checked {
    // Templateの名前は、作り直すか使い回すかの判断に使う。読めない一覧を空として
    // 扱うと、在るTemplateをもう一度作る。何が読めなかったかを分けて示す。
    for (output, cause) in [
        (
            r#"["sbxm-example-template:v1"]"#,
            "an entry is not an object",
        ),
        (
            r#"[{"id":"a3d0f4449170","tag":"v1"}]"#,
            "an entry has no repository",
        ),
        (
            r#"[{"repository":"","tag":"v1"}]"#,
            "an entry has no repository",
        ),
        (
            r#"[{"repository":"docker.io/library/x"}]"#,
            "the entry for docker.io/library/x has no tag",
        ),
        ("12", "the document is not an array"),
    ] {
        let error = parse_template_list(output).refused_because("a listing that cannot be read")?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
        assert_eq!(refusal_cause(&error)?, cause, "output {output}");
    }
    Ok(())
}
