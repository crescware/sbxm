use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::compatibility::parse_template_list;
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
fn the_sandbox_list_parser_reads_the_fields_the_workflow_compares() -> Checked {
    let output = r#"{"sandboxes":[{"name":"sbxm-a","status":"running","workspaces":["/tmp/docker-sandboxes/sbxm-a"]}]}"#;
    let entries = parse_sandbox_list(output).required_because("a listing parses")?;
    assert_eq!(
        entries,
        vec![SandboxEntry {
            name: "sbxm-a".to_string(),
            state: SandboxState::Running,
            raw_state: "running".to_string(),
            workspace: Some("/tmp/docker-sandboxes/sbxm-a".to_string()),
        }]
    );

    assert!(
        parse_sandbox_list("  \n")
            .required_because("an empty listing")?
            .is_empty()
    );

    // 3値へ写像しても、runtimeが示したままの値は表示のために残す。
    let entries =
        parse_sandbox_list(r#"{"sandboxes":[{"name":"sbxm-a","status":"Running"}]}"#).required()?;
    assert_eq!(entries[0].state, SandboxState::Running);
    assert_eq!(entries[0].raw_state, "Running");
    Ok(())
}

#[test]
fn a_form_with_no_confirmed_sbx_version_is_rejected() -> Checked {
    // sbx v0.37.0（要件となる最小version）は常に`{"sandboxes": [...]}`で包み、
    // item は`status`と配列の`workspaces`で示す。それ以外の形を出す実versionの根拠が
    // ないため、包みのない配列・1行1件のJSON・`state`fieldは受け付けない。
    for (output, cause) in [
        (
            r#"[{"name":"sbxm-a","status":"running"}]"#,
            "the document does not wrap sandboxes",
        ),
        (
            r#"{"name":"sbxm-a","status":"running"}"#,
            "the document has no sandboxes field",
        ),
        (
            r#"{"sandboxes":[{"name":"sbxm-a","state":"running"}]}"#,
            "sandbox sbxm-a has no state",
        ),
    ] {
        let error = parse_sandbox_list(output)
            .refused_because("a form with no confirmed sbx version is rejected")?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
        assert_eq!(refusal_cause(&error)?, cause, "output {output}");
    }

    // 1行1件のJSONは、包んだ形として1度に読めないためparse自体が失敗する。
    let lines = "{\"name\":\"sbxm-a\",\"status\":\"running\"}\n{\"name\":\"sbxm-b\",\"status\":\"running\"}\n";
    let error = parse_sandbox_list(lines)
        .refused_because("line-delimited output is not the wrapped document sbx returns")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    Ok(())
}

#[test]
fn the_sandbox_state_has_one_untranslated_spelling() {
    assert_eq!(SandboxState::Running.as_str(), "running");
    assert_eq!(SandboxState::Stopped.as_str(), "stopped");
}

#[test]
fn the_listing_of_the_target_version_is_read_as_it_is() -> Checked {
    // 対象versionが実際に出力する形。`sandboxes`で包み、workspaceは配列で示す。
    let observed = r#"{
  "sandboxes": [
    {
      "name": "crescware-sbxm",
      "id": "ec55cefe-9919-4c0e-952c-db88e5466db2",
      "agent": "shell",
      "status": "running",
      "workspaces": [
        "/tmp/docker-sandboxes/crescware-sbxm"
      ]
    },
    {
      "name": "okunokentaro-inventory",
      "id": "ebd3a9e1-ac6a-40fd-9ebc-6531fd824f7c",
      "agent": "shell",
      "status": "stopped",
      "workspaces": [
        "/tmp/docker-sandboxes/okunokentaro-inventory"
      ]
    }
  ]
}"#;

    let entries = parse_sandbox_list(observed).required_because("the real listing parses")?;
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "crescware-sbxm");
    assert_eq!(entries[0].state, SandboxState::Running);
    assert_eq!(
        entries[0].workspace.as_deref(),
        Some("/tmp/docker-sandboxes/crescware-sbxm")
    );
    assert_eq!(entries[1].state, SandboxState::Stopped);

    // Sandboxが1件もない場合。
    assert!(
        parse_sandbox_list(r#"{"sandboxes": []}"#)
            .required_because("an empty listing")?
            .is_empty()
    );
    Ok(())
}

#[test]
fn a_null_listing_is_read_as_nothing_rather_than_refused() -> Checked {
    assert!(
        parse_sandbox_list(r#"{"sandboxes": null}"#)
            .required_because("a null sandbox listing")?
            .is_empty()
    );
    assert!(
        parse_template_list(r#"{"images": null}"#)
            .required_because("a null template listing")?
            .is_empty()
    );
    Ok(())
}

#[test]
fn a_sandbox_with_more_than_one_workspace_is_not_guessed_at() -> Checked {
    let two =
        r#"{"sandboxes":[{"name":"sbxm-a","status":"running","workspaces":["/tmp/a","/tmp/b"]}]}"#;
    let error = parse_sandbox_list(two).refused_because("one of two workspaces is not chosen")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));

    let none = r#"{"sandboxes":[{"name":"sbxm-a","status":"running","workspaces":[]}]}"#;
    let entries = parse_sandbox_list(none).required_because("an empty list is observable")?;
    assert_eq!(entries[0].workspace, None);
    Ok(())
}

#[test]
fn a_workspace_field_that_is_not_a_list_of_paths_is_not_guessed_at() -> Checked {
    // workspaceはこの案件の成果物かどうかの判定に使う。示し方が変わった一覧から
    // pathらしき値を拾うと、無関係のSandboxを案件のものとして片付けにかける。
    let absent = r#"{"sandboxes":[{"name":"sbxm-a","status":"running","workspaces":null}]}"#;
    let entries =
        parse_sandbox_list(absent).required_because("a null list is an observable absence")?;
    assert_eq!(entries[0].workspace, None);

    for (output, cause) in [
        (
            r#"{"sandboxes":[{"name":"sbxm-a","status":"running","workspaces":"/tmp/a"}]}"#,
            "workspaces is not a list",
        ),
        (
            r#"{"sandboxes":[{"name":"sbxm-a","status":"running","workspaces":[7]}]}"#,
            "a workspace is not a string",
        ),
    ] {
        let error = parse_sandbox_list(output).refused_because("a workspace that is not a path")?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
        assert_eq!(refusal_cause(&error)?, cause, "output {output}");
    }
    Ok(())
}

#[test]
fn a_sandbox_listing_that_cannot_be_read_states_which_reading_failed() -> Checked {
    // state不明のSandboxを止まっているものとして扱うと、動いているSandboxを消す。
    // どこで読めなくなったかを、拒否ごとに分けて示す。
    for (output, cause) in [
        (r#"{"sandboxes":["sbxm-a"]}"#, "an entry is not an object"),
        (
            r#"{"sandboxes":[{"status":"running"}]}"#,
            "an entry has no name",
        ),
        (
            r#"{"sandboxes":[{"name":"","status":"running"}]}"#,
            "an entry has no name",
        ),
        (
            r#"{"sandboxes":[{"name":"sbxm-a"}]}"#,
            "sandbox sbxm-a has no state",
        ),
        (
            r#"{"sandboxes":[{"name":"sbxm-a","status":"pausing"}]}"#,
            "state pausing has no defined meaning in this build",
        ),
        ("true", "the document does not wrap sandboxes"),
    ] {
        let error = parse_sandbox_list(output).refused_because("a listing that cannot be read")?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
        assert_eq!(refusal_cause(&error)?, cause, "output {output}");
    }
    Ok(())
}
