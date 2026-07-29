use super::*;
use crate::compatibility::parse_template_list;
use crate::error::ErrorId;

#[test]
fn the_sandbox_list_parser_reads_the_fields_the_workflow_compares() {
    let output =
        r#"[{"name":"sbxm-a","state":"running","workspace":"/tmp/docker-sandboxes/sbxm-a"}]"#;
    let entries = parse_sandbox_list(output).expect("a listing parses");
    assert_eq!(
        entries,
        vec![SandboxEntry {
            name: "sbxm-a".to_string(),
            state: SandboxState::Running,
            raw_state: "running".to_string(),
            workspace: Some("/tmp/docker-sandboxes/sbxm-a".to_string()),
        }]
    );

    // 1行1件のJSONと、空の出力も同じ意味で読む。
    let lines = "{\"name\":\"sbxm-a\",\"state\":\"stopped\"}\n{\"name\":\"sbxm-b\",\"status\":\"running\"}\n";
    let entries = parse_sandbox_list(lines).expect("line-delimited output parses");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].state, SandboxState::Stopped);
    assert_eq!(entries[1].state, SandboxState::Running);
    assert!(
        parse_sandbox_list("  \n")
            .expect("an empty listing")
            .is_empty()
    );

    // 3値へ写像しても、runtimeが示したままの値は表示のために残す。
    let entries = parse_sandbox_list(r#"[{"name":"sbxm-a","state":"Running"}]"#).unwrap();
    assert_eq!(entries[0].state, SandboxState::Running);
    assert_eq!(entries[0].raw_state, "Running");
}

#[test]
fn the_sandbox_state_has_one_untranslated_spelling() {
    assert_eq!(SandboxState::Running.as_str(), "running");
    assert_eq!(SandboxState::Stopped.as_str(), "stopped");
}

#[test]
fn the_listing_of_the_target_version_is_read_as_it_is() {
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

    let entries = parse_sandbox_list(observed).expect("the real listing parses");
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
            .expect("an empty listing")
            .is_empty()
    );
}

#[test]
fn a_null_listing_is_read_as_nothing_rather_than_refused() {
    assert!(
        parse_sandbox_list(r#"{"sandboxes": null}"#)
            .expect("a null sandbox listing")
            .is_empty()
    );
    assert!(
        parse_template_list(r#"{"images": null}"#)
            .expect("a null template listing")
            .is_empty()
    );
}

#[test]
fn a_sandbox_with_more_than_one_workspace_is_not_guessed_at() {
    let two =
        r#"{"sandboxes":[{"name":"sbxm-a","status":"running","workspaces":["/tmp/a","/tmp/b"]}]}"#;
    let error = parse_sandbox_list(two).expect_err("one of two workspaces is not chosen");
    assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));

    let none = r#"{"sandboxes":[{"name":"sbxm-a","status":"running","workspaces":[]}]}"#;
    let entries = parse_sandbox_list(none).expect("an empty list is observable");
    assert_eq!(entries[0].workspace, None);
}

#[test]
fn a_sandbox_listing_that_cannot_be_read_is_refused() {
    for output in [
        r#"[{"state":"running"}]"#,
        r#"[{"name":"sbxm-a"}]"#,
        r#"[{"name":"sbxm-a","state":"pausing"}]"#,
        r#"["sbxm-a"]"#,
        "true",
    ] {
        let error = parse_sandbox_list(output).expect_err("{output} must be refused");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::ExternalOutputUnparseable),
            "output {output} produced the wrong error"
        );
    }
}
