use super::*;

fn location() -> (tempfile::TempDir, ConfigLocation) {
    let dir = tempfile::tempdir().expect("temporary home");
    let location = ConfigLocation::from_home(dir.path().to_path_buf());
    (dir, location)
}

fn valid_config_text(base_path: &Path) -> String {
    format!(
        r#"version: 1
language: ja
base_path: "{}"

git:
  user_name: Example User
  user_email: user@example.com
"#,
        base_path.display()
    )
}

fn write_config(location: &ConfigLocation, text: &str) {
    let dir = location.dir();
    fs::create_dir_all(&dir).expect("create config dir");
    fs::set_permissions(&dir, fs::Permissions::from_mode(PRIVATE_DIR_MODE)).expect("mode");
    let path = location.config_file();
    fs::write(&path, text).expect("write config");
    fs::set_permissions(&path, fs::Permissions::from_mode(PRIVATE_FILE_MODE)).expect("mode");
}

#[test]
fn a_missing_configuration_is_reported_as_missing_rather_than_failing() {
    let (_dir, location) = location();
    assert!(matches!(
        load(&location).expect("missing is not an error"),
        ConfigState::Missing
    ));
}

#[test]
fn configuration_paths_follow_the_documented_layout() {
    let location = ConfigLocation::from_home(PathBuf::from("/Users/example"));
    assert_eq!(location.dir(), PathBuf::from("/Users/example/.sbxm"));
    assert_eq!(
        location.config_file(),
        PathBuf::from("/Users/example/.sbxm/config.yaml")
    );
    assert_eq!(
        location.init_lock(),
        PathBuf::from("/Users/example/.sbxm/init.lock")
    );
}

#[test]
fn a_valid_configuration_round_trips_through_render_and_load() {
    let (dir, location) = location();
    let base = dir.path().join("Projects");
    fs::create_dir_all(&base).unwrap();

    let config = GlobalConfig {
        language: Locale::Ja,
        base_path: AbsoluteBasePath::new(&base).unwrap(),
        git: GitIdentity {
            user_name: "Example User".into(),
            user_email: "user@example.com".into(),
        },
        files: vec![FileDeclaration {
            source: HostFileSource::new("/Users/example/.gitconfig").unwrap(),
            destination: SandboxHomeRelativePath::new(".gitconfig").unwrap(),
        }],
    };

    ensure_config_dir(&location).unwrap();
    create(&location, &config).unwrap();

    let ConfigState::Valid {
        config: loaded,
        warnings,
    } = load(&location).expect("the written configuration loads")
    else {
        panic!("the configuration must be present after create");
    };
    assert_eq!(*loaded, config);
    assert!(warnings.is_empty());
}

#[test]
fn the_created_configuration_is_private_to_its_owner() {
    let (dir, location) = location();
    let base = dir.path().join("Projects");
    let config = GlobalConfig {
        language: Locale::En,
        base_path: AbsoluteBasePath::new(&base).unwrap(),
        git: GitIdentity {
            user_name: "Example User".into(),
            user_email: "user@example.com".into(),
        },
        files: Vec::new(),
    };
    ensure_config_dir(&location).unwrap();
    let path = create(&location, &config).unwrap();

    let file_mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(file_mode, 0o600);
    let dir_mode = fs::metadata(location.dir()).unwrap().permissions().mode() & 0o777;
    assert_eq!(dir_mode, 0o700);
}

#[test]
fn creating_a_configuration_twice_does_not_overwrite_the_first() {
    let (dir, location) = location();
    let config = GlobalConfig {
        language: Locale::En,
        base_path: AbsoluteBasePath::new(&dir.path().join("Projects")).unwrap(),
        git: GitIdentity {
            user_name: "Example User".into(),
            user_email: "user@example.com".into(),
        },
        files: Vec::new(),
    };
    ensure_config_dir(&location).unwrap();
    create(&location, &config).unwrap();
    let error = create(&location, &config).expect_err("the second create must refuse");
    assert_eq!(error.first_id(), Some(ErrorId::TargetAppearedConcurrently));
}

#[test]
fn invalid_syntax_is_reported_with_the_path() {
    let (_dir, location) = location();
    // 閉じられていない引用符。scalarの途中でstreamが終わる。
    write_config(&location, "version: 1\nlanguage: \"ja\n");
    let error = load(&location).expect_err("broken YAML fails to load");
    assert_eq!(error.first_id(), Some(ErrorId::ConfigInvalidSyntax));
}

#[test]
fn an_empty_configuration_names_the_field_it_lacks() {
    let (_dir, location) = location();
    // 空のdocumentはnullとして読める。syntax errorではなく欠落として報告する。
    write_config(&location, "");
    let error = load(&location).expect_err("an empty configuration fails to load");
    assert_eq!(error.first_id(), Some(ErrorId::ConfigMissingField));
}

#[test]
fn an_unknown_version_is_diagnosed_before_other_fields() {
    let (_dir, location) = location();
    write_config(&location, "version: 99\n");
    let error = load(&location).expect_err("unknown versions fail to load");
    assert_eq!(error.first_id(), Some(ErrorId::ConfigUnknownVersion));
}

#[test]
fn missing_required_fields_are_named() {
    let (dir, location) = location();
    let cases = [
        ("version: 1\n", "language"),
        ("version: 1\nlanguage: en\n", "base_path"),
        (
            &format!(
                "version: 1\nlanguage: en\nbase_path: \"{}\"\n",
                dir.path().display()
            ),
            "git",
        ),
    ];
    for (text, _field) in cases {
        write_config(&location, text);
        let error = load(&location).expect_err("incomplete configurations fail");
        assert_eq!(error.first_id(), Some(ErrorId::ConfigMissingField));
    }
}

#[test]
fn unknown_top_level_keys_are_warnings_in_version_1() {
    let (dir, location) = location();
    // top-levelのkeyとして解釈させるため、字下げせずに置く。
    let text =
        valid_config_text(dir.path()).replace("language: ja", "language: ja\nfuture_option: true");
    write_config(&location, &text);

    let ConfigState::Valid { warnings, .. } = load(&location).expect("unknown keys still load")
    else {
        panic!("the configuration must load");
    };
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].id, "warning-config-unknown-key");
}

#[test]
fn an_unsupported_language_is_rejected() {
    let (dir, location) = location();
    // 組み込みlocaleにならないtagを使う。
    let text = valid_config_text(dir.path()).replace("language: ja", "language: zz");
    write_config(&location, &text);
    let error = load(&location).expect_err("unsupported languages fail");
    assert_eq!(error.first_id(), Some(ErrorId::ConfigInvalidValue));
}

#[test]
fn a_relative_base_path_is_rejected() {
    let (dir, location) = location();
    let text = valid_config_text(dir.path()).replace(
        &format!("\"{}\"", dir.path().display()),
        "\"relative/projects\"",
    );
    write_config(&location, &text);
    let error = load(&location).expect_err("relative base paths fail");
    assert_eq!(error.first_id(), Some(ErrorId::BasePathNotAbsolute));
}

#[test]
fn an_over_permissive_configuration_is_refused_and_not_repaired() {
    let (dir, location) = location();
    write_config(&location, &valid_config_text(dir.path()));
    let path = location.config_file();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();

    let error = load(&location).expect_err("world-readable configurations are refused");
    assert_eq!(error.first_id(), Some(ErrorId::ConfigPermissionTooOpen));
    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o644, "sbxm must not repair permissions on its own");
}

#[test]
fn a_symlinked_configuration_is_refused() {
    let (dir, location) = location();
    fs::create_dir_all(location.dir()).unwrap();
    let real = dir.path().join("real-config.yaml");
    fs::write(&real, valid_config_text(dir.path())).unwrap();
    std::os::unix::fs::symlink(&real, location.config_file()).unwrap();

    let error = load(&location).expect_err("symlinked configurations are refused");
    assert_eq!(error.first_id(), Some(ErrorId::ConfigSymlink));
}

#[test]
fn declared_file_sources_must_be_absolute() {
    let (dir, location) = location();
    let mut text = valid_config_text(dir.path());
    text.push_str("\nfiles:\n  - source: relative/file\n    destination: .config/x\n");
    write_config(&location, &text);

    let error = load(&location).expect_err("relative sources are refused");
    assert_eq!(
        error.first_id(),
        Some(ErrorId::FileDeclarationInvalidSource)
    );
}

#[test]
fn declared_file_destinations_must_stay_under_the_sandbox_home() {
    let (dir, location) = location();
    for destination in ["/etc/passwd", "../outside", "nested/../../outside"] {
        let mut text = valid_config_text(dir.path());
        text.push_str(&format!(
            "\nfiles:\n  - source: /tmp/source\n    destination: \"{destination}\"\n"
        ));
        write_config(&location, &text);

        let error = load(&location).expect_err("{destination} must be refused");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::FileDeclarationInvalidDestination),
            "destination {destination} produced the wrong error"
        );
    }
}

#[test]
fn git_identity_values_reject_empty_and_multi_line_input() {
    assert!(validate_git_identity_value("Example User").is_ok());
    assert_eq!(validate_git_identity_value(""), Err("detail-value-empty"));
    assert_eq!(
        validate_git_identity_value("   "),
        Err("detail-value-empty")
    );
    assert_eq!(
        validate_git_identity_value("Example\nUser"),
        Err("detail-value-has-newline")
    );
}

/// sbxmのvalidationは通るが、YAMLとしては別の型や構造に読めてしまう値。
///
/// git identityは空文字と改行しか拒まないため、これらはすべて設定に現れ得る。
fn yaml_lookalike_values() -> Vec<String> {
    let mut values: Vec<String> = [
        "no",
        "yes",
        "on",
        "off",
        "true",
        "false",
        "null",
        "~",
        "123",
        "1.0",
        "0755",
        "#hash",
        "a: b",
        "- item",
        "? question",
        "*alias",
        "&anchor",
        "!tag",
        "%directive",
        "@reserved",
        "`backtick",
        "  padded  ",
        "tab\there",
        "quote\"inside",
        "'single'",
        "日本語 🙂",
    ]
    .iter()
    .map(|value| (*value).to_string())
    .collect();
    // emitterは長い行を折り返す。折り返しても値は変わらない。
    values.push("Example User ".repeat(40).trim_end().to_string());
    values
}

#[test]
fn rendered_values_survive_a_round_trip_even_when_they_look_like_yaml_syntax() {
    let dir = tempfile::tempdir().expect("temporary base");
    let base = AbsoluteBasePath::new(dir.path()).unwrap();

    for value in yaml_lookalike_values() {
        let config = GlobalConfig {
            language: Locale::En,
            base_path: base.clone(),
            git: GitIdentity {
                user_name: value.clone(),
                user_email: format!("{value}@example.com"),
            },
            files: vec![FileDeclaration {
                source: HostFileSource::new(&format!("/hosts/{value}")).unwrap(),
                destination: SandboxHomeRelativePath::new(&format!(".config/{value}")).unwrap(),
            }],
        };

        let rendered = render(&config);
        let state = parse(&rendered, Path::new("/tmp/config.yaml")).unwrap_or_else(|error| {
            panic!("{value:?} rendered YAML that does not parse: {error:?}\n{rendered}")
        });
        let ConfigState::Valid {
            config: loaded,
            warnings,
        } = state
        else {
            panic!("{value:?} must render a complete configuration");
        };
        assert!(warnings.is_empty(), "{value:?} produced {warnings:?}");
        assert_eq!(*loaded, config, "{value:?} did not survive the round trip");
    }
}

#[test]
fn rendering_quotes_values_that_need_escaping() {
    let config = GlobalConfig {
        language: Locale::En,
        base_path: AbsoluteBasePath::new(Path::new("/Users/ex ample")).unwrap(),
        git: GitIdentity {
            user_name: "Quote \" User".into(),
            user_email: "user@example.com".into(),
        },
        files: Vec::new(),
    };
    let rendered = render(&config);
    let reparsed: yaml_serde::Value =
        yaml_serde::from_str(&rendered).expect("rendered config is YAML");
    assert_eq!(reparsed["git"]["user_name"].as_str(), Some("Quote \" User"));
    assert_eq!(reparsed["base_path"].as_str(), Some("/Users/ex ample"));
}
