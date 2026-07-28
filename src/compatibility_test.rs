use super::*;

#[test]
fn versions_require_exactly_three_numeric_parts() {
    assert_eq!(
        CliVersion::parse("0.37.0"),
        Some(CliVersion {
            major: 0,
            minor: 37,
            patch: 0
        })
    );
    assert_eq!(CliVersion::parse("0.37"), None);
    assert_eq!(CliVersion::parse("0.37.0.1"), None);
    assert_eq!(CliVersion::parse("0.37.x"), None);
    assert_eq!(CliVersion::parse(""), None);
}

#[test]
fn versions_are_extracted_from_surrounding_text() {
    assert_eq!(
        CliVersion::extract_from_output("sbx version 0.37.2\n"),
        CliVersion::parse("0.37.2")
    );
    assert_eq!(
        CliVersion::extract_from_output("Docker Sandboxes CLI v1.2.3 (build abc)"),
        CliVersion::parse("1.2.3")
    );
    assert_eq!(CliVersion::extract_from_output("no version here"), None);
    assert_eq!(CliVersion::extract_from_output(""), None);
}

#[test]
fn versions_below_the_minimum_are_refused() {
    let error = require_minimum_version(CliVersion::parse("0.36.9").unwrap())
        .expect_err("an older version must be refused");
    assert_eq!(error.first_id(), Some(ErrorId::SbxVersionBelowMinimum));
}

#[test]
fn the_minimum_version_and_later_are_accepted() {
    for observed in ["0.37.0", "0.37.5", "0.38.0", "1.0.0"] {
        assert!(
            require_minimum_version(CliVersion::parse(observed).unwrap()).is_ok(),
            "{observed} must be accepted"
        );
    }
}

#[test]
fn the_daemon_status_parser_reads_the_status_line_of_the_real_output() {
    // 対象versionが実際に出力する形。socketとlogのpathは読まない。
    let observed = "Status: running\nSocket: /Users/<user>/Library/Application Support/com.docker.sandboxes/sandboxes/sandboxd/sandboxd.sock\nLogs: /Users/<user>/Library/Application Support/com.docker.sandboxes/sandboxes/sandboxd/daemon.log\n";
    assert_eq!(parse_daemon_status(observed).unwrap(), DaemonState::Running);

    assert_eq!(
        parse_daemon_status("Status: stopped\n").unwrap(),
        DaemonState::Stopped
    );
    assert_eq!(
        parse_daemon_status("Status: Running\n").unwrap(),
        DaemonState::Running
    );

    for output in [
        "",
        "Socket: /tmp/sandboxd.sock\n",
        "Status: degraded\n",
        r#"{"running": true}"#,
    ] {
        let error = parse_daemon_status(output).expect_err("unknown states are not guessed");
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    }
}

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

#[test]
fn the_template_listing_of_the_target_version_is_read_as_it_is() {
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

    let entries = parse_template_list(observed).expect("the real listing parses");
    assert_eq!(entries.len(), 1);

    // sbxmが渡す名前はregistry prefixを持たない。runtimeは補って表示する。
    assert!(
        entries[0].is_named("sbxm-example-org-example-repo-0123456789ab-template:548a91cfab02")
    );
    assert!(entries[0].is_named(
        "docker.io/library/sbxm-example-org-example-repo-0123456789ab-template:548a91cfab02"
    ));
    assert!(!entries[0].is_named("sbxm-example-org-example-repo-0123456789ab-template:other"));

    assert!(parse_template_list(r#"{"images": []}"#).unwrap().is_empty());
    assert!(parse_template_list("").unwrap().is_empty());

    // repositoryとtagのどちらかを欠く一覧からは、対応を決められない。
    for output in [
        r#"{"images":[{"id":"a3d0f4449170","tag":"v1"}]}"#,
        r#"{"images":[{"id":"a3d0f4449170","repository":"docker.io/library/x"}]}"#,
        r#"{"images":[{"repository":"","tag":"v1"}]}"#,
        "12",
    ] {
        let error = parse_template_list(output).expect_err("{output} must be refused");
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    }
}

#[test]
fn the_secret_listing_of_the_target_version_is_read_as_it_is() {
    // 対象versionが実際に出力する形。service secretの表のあとに、見出しを挟んで
    // custom secretの表が続く。
    let observed = "SCOPE           TYPE      NAME     SECRET\n\
                        sbxm-example    service   github   (stored)\n\
                        \n\
                        CUSTOM SECRETS\n\
                        SCOPE          TARGETS      ENV        PLACEHOLDER      SECRET\n\
                        sbxm-example   github.com   GH_TOKEN   sbx-cs-example   ghp_example\n";
    assert_eq!(
        parse_custom_secrets(observed).unwrap(),
        vec![CustomSecret {
            placeholder: "sbx-cs-example".to_string(),
            targets: vec!["github.com".to_string()],
            env: "GH_TOKEN".to_string(),
        }]
    );

    // custom secretの見出しがない出力は、1件も登録がないことを示す。service secretの
    // 表だけを読み違えて登録ありとしない。
    let services_only = "SCOPE           TYPE      NAME     SECRET\n\
                             sbxm-example    service   github   (stored)\n";
    assert!(parse_custom_secrets(services_only).unwrap().is_empty());

    // 1件もない場合は表ではなく文で示す。
    let absent = "No secrets found for scope \"sbxm-example\".\n";
    assert!(parse_custom_secrets(absent).unwrap().is_empty());

    // 1つの列が複数のhostを並べることがある。空白1つで切ると列がずれる。
    let several = "CUSTOM SECRETS\n\
                       SCOPE          TARGETS                  ENV        PLACEHOLDER      SECRET\n\
                       sbxm-example   github.com gitlab.com    GH_TOKEN   sbx-cs-example   ghp_example\n";
    assert_eq!(
        parse_custom_secrets(several).unwrap()[0].targets,
        vec!["github.com".to_string(), "gitlab.com".to_string()]
    );

    // 実機がwildcardを登録したscopeで出す形。`TARGETS`はcommaと空白1つで区切り、
    // wildcardは展開せず書いたまま並べる。scope名とsecretは記録から伏せてある。
    let wildcards = "CUSTOM SECRETS\n\
                         SCOPE          TARGETS                                                        ENV        PLACEHOLDER               SECRET\n\
                         sbxm-example   github.com, **.github.com, **.githubusercontent.com, ghcr.io   GH_TOKEN   sbx-cs-Y1k0SfTWbkN6HzCO   ghp_redacted\n";
    let parsed = parse_custom_secrets(wildcards).unwrap();
    assert_eq!(
        parsed,
        vec![CustomSecret {
            targets: vec![
                "github.com".to_string(),
                "**.github.com".to_string(),
                "**.githubusercontent.com".to_string(),
                "ghcr.io".to_string(),
            ],
            env: "GH_TOKEN".to_string(),
            placeholder: "sbx-cs-Y1k0SfTWbkN6HzCO".to_string(),
        }],
        "the pattern is compared as written, so it has to survive the listing unexpanded"
    );

    for output in [
        "",
        "CUSTOM SECRETS\nSCOPE          ENV\nsbxm-example   GH_TOKEN\n",
        "CUSTOM SECRETS\nSCOPE          TARGETS      ENV\nsbxm-example   github.com\n",
    ] {
        let error = parse_custom_secrets(output).expect_err("{output} must be refused");
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    }
}

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

#[test]
fn the_network_policy_parser_reads_the_active_entry_only() {
    let balanced = r#"[{"name":"Balanced","active":true},{"name":"Open","active":false}]"#;
    assert_eq!(parse_network_policy(balanced).unwrap(), "Balanced");

    let other = r#"[{"name":"Balanced","active":false},{"name":"Open","active":true}]"#;
    assert_ne!(
        parse_network_policy(other).unwrap(),
        EXPECTED_NETWORK_POLICY
    );

    for output in [
        "{}",
        r#"[{"name":"Balanced","active":false}]"#,
        r#"[{"name":"Balanced","active":true},{"name":"Open","active":true}]"#,
    ] {
        let error = parse_network_policy(output).expect_err("an ambiguous policy is not guessed");
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    }
}
