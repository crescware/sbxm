use crate::diagnostics::ErrorId;
use crate::i18n::Locale;
use crate::metadata::GitIdentity;
use crate::paths::{PRIVATE_DIR_MODE, PRIVATE_FILE_MODE};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::testing::outcome::{Checked, Refused, Required, Unmet};

use super::*;

fn location() -> Checked<(tempfile::TempDir, ConfigLocation)> {
    let dir = tempfile::tempdir().required_because("temporary home")?;
    let location = ConfigLocation::from_home(dir.path().to_path_buf());
    Ok((dir, location))
}

fn valid_config_text() -> String {
    "version: 1\nlanguage: ja\n".to_string()
}

fn write_config(location: &ConfigLocation, text: &str) -> Checked {
    let dir = location.dir();
    fs::create_dir_all(&dir).required_because("create config dir")?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(PRIVATE_DIR_MODE))
        .required_because("mode")?;
    let path = location.config_file();
    fs::write(&path, text).required_because("write config")?;
    fs::set_permissions(&path, fs::Permissions::from_mode(PRIVATE_FILE_MODE))
        .required_because("mode")?;
    Ok(())
}

fn loaded(location: &ConfigLocation) -> Checked<GlobalConfig> {
    let ConfigState::Valid { config, .. } =
        load(location).required_because("the configuration loads")?
    else {
        return Err(Unmet::new("the configuration must be present".to_string()));
    };
    Ok(*config)
}

#[test]
fn a_missing_configuration_is_reported_as_missing_rather_than_failing() -> Checked {
    let (_dir, location) = location()?;
    assert!(matches!(
        load(&location).required_because("missing is not an error")?,
        ConfigState::Missing
    ));
    // 不在は正常であり、defaultとして扱う。
    assert_eq!(
        load(&location).required()?.settings(),
        GlobalConfig::default()
    );
    Ok(())
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
        location.registry_file(),
        PathBuf::from("/Users/example/.sbxm/registry.yaml")
    );
    assert_eq!(
        location.registry_lock(),
        PathBuf::from("/Users/example/.sbxm/registry.lock")
    );
}

#[test]
fn a_configuration_that_only_declares_its_version_is_valid() -> Checked {
    let (_dir, location) = location()?;
    write_config(&location, "version: 1\n")?;

    let config = loaded(&location)?;
    assert_eq!(config.language, None, "an absent language is unsaved");
    assert!(config.files.is_empty());
    Ok(())
}

#[test]
fn a_valid_configuration_round_trips_through_render_and_load() -> Checked {
    let (_dir, location) = location()?;
    let config = GlobalConfig {
        language: Some(Locale::Ja),
        git_identity: Some(GitIdentity {
            user_name: "Example User".to_string(),
            user_email: "user@example.com".to_string(),
        }),
        files: vec![FileDeclaration {
            source: HostFileSource::new("/Users/example/.gitconfig").required()?,
            destination: SandboxHomeRelativePath::new(".gitconfig").required()?,
        }],
    };

    write_config(&location, &render(&config)?)?;
    assert_eq!(loaded(&location)?, config);
    Ok(())
}

#[test]
fn the_language_is_saved_without_rewriting_what_the_user_wrote() -> Checked {
    let (_dir, location) = location()?;
    let handwritten = "\
# my settings
version: 1

files:
  - source: /Users/example/.gitconfig
    destination: .gitconfig
";
    write_config(&location, handwritten)?;

    let path = save_language(&location, Locale::Ja).required_because("the language is saved")?;
    let written = fs::read_to_string(&path).required()?;
    assert_eq!(
        written,
        "\
# my settings
version: 1
language: ja

files:
  - source: /Users/example/.gitconfig
    destination: .gitconfig
",
        "comments, blank lines, key order and files are kept"
    );
    assert_eq!(loaded(&location)?.language, Some(Locale::Ja));

    // 二度目は`language`行だけを差し替える。
    save_language(&location, Locale::En).required_because("the language is replaced")?;
    let written = fs::read_to_string(&path).required()?;
    assert!(written.contains("language: en"), "{written}");
    assert!(!written.contains("language: ja"), "{written}");
    assert!(written.starts_with("# my settings\n"), "{written}");
    assert!(written.contains(".gitconfig"), "{written}");
    Ok(())
}

#[test]
fn saving_the_language_creates_a_private_configuration_when_there_is_none() -> Checked {
    let (_dir, location) = location()?;
    let path =
        save_language(&location, Locale::Ja).required_because("the configuration is created")?;

    let file_mode = fs::metadata(&path).required()?.permissions().mode() & 0o777;
    assert_eq!(file_mode, 0o600);
    let dir_mode = fs::metadata(location.dir())
        .required()?
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(dir_mode, 0o700);
    assert_eq!(loaded(&location)?.language, Some(Locale::Ja));
    Ok(())
}

#[test]
fn invalid_syntax_is_reported_with_the_path() -> Checked {
    let (_dir, location) = location()?;
    // 閉じられていない引用符。scalarの途中でstreamが終わる。
    write_config(&location, "version: 1\nlanguage: \"ja\n")?;
    let error = load(&location).refused_because("broken YAML fails to load")?;
    assert_eq!(error.first_id(), Some(ErrorId::ConfigInvalidSyntax));
    Ok(())
}

#[test]
fn an_empty_configuration_names_the_field_it_lacks() -> Checked {
    let (_dir, location) = location()?;
    // 空のdocumentはnullとして読める。syntax errorではなく欠落として報告する。
    write_config(&location, "")?;
    let error = load(&location).refused_because("an empty configuration fails to load")?;
    assert_eq!(error.first_id(), Some(ErrorId::ConfigMissingField));
    Ok(())
}

#[test]
fn an_unknown_version_is_diagnosed_before_other_fields() -> Checked {
    let (_dir, location) = location()?;
    write_config(&location, "version: 99\n")?;
    let error = load(&location).refused_because("unknown versions fail to load")?;
    assert_eq!(error.first_id(), Some(ErrorId::ConfigUnknownVersion));
    Ok(())
}

#[test]
fn unknown_top_level_keys_are_warnings_in_version_1() -> Checked {
    let (_dir, location) = location()?;
    // top-levelのkeyとして解釈させるため、字下げせずに置く。
    let text = valid_config_text().replace("language: ja", "language: ja\nfuture_option: true");
    write_config(&location, &text)?;

    let ConfigState::Valid { warnings, .. } =
        load(&location).required_because("unknown keys still load")?
    else {
        return Err(Unmet::new("the configuration must load".to_string()));
    };
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].description.id, "warning-config-unknown-key");
    Ok(())
}

#[test]
fn an_unsupported_language_is_rejected() -> Checked {
    let (_dir, location) = location()?;
    // 組み込みlocaleにならないtagを使う。欠落と不正な値は別の状態である。
    let text = valid_config_text().replace("language: ja", "language: zz");
    write_config(&location, &text)?;
    let error = load(&location).refused_because("unsupported languages fail")?;
    assert_eq!(error.first_id(), Some(ErrorId::ConfigInvalidValue));
    Ok(())
}

#[test]
fn an_over_permissive_configuration_is_refused_and_not_repaired() -> Checked {
    let (_dir, location) = location()?;
    write_config(&location, &valid_config_text())?;
    let path = location.config_file();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).required()?;

    let error = load(&location).refused_because("world-readable configurations are refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ConfigPermissionTooOpen));
    let mode = fs::metadata(&path).required()?.permissions().mode() & 0o777;
    assert_eq!(mode, 0o644, "sbxm must not repair permissions on its own");
    Ok(())
}

#[test]
fn a_symlinked_configuration_is_refused() -> Checked {
    let (dir, location) = location()?;
    fs::create_dir_all(location.dir()).required()?;
    fs::set_permissions(location.dir(), fs::Permissions::from_mode(PRIVATE_DIR_MODE)).required()?;
    let real = dir.path().join("real-config.yaml");
    fs::write(&real, valid_config_text()).required()?;
    std::os::unix::fs::symlink(&real, location.config_file()).required()?;

    let error = load(&location).refused_because("symlinked configurations are refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ConfigSymlink));
    let error = save_language(&location, Locale::Ja)
        .refused_because("a symlinked configuration is never written through")?;
    assert_eq!(error.first_id(), Some(ErrorId::ConfigSymlink));
    Ok(())
}

#[test]
fn declared_file_sources_must_be_absolute() -> Checked {
    let (_dir, location) = location()?;
    let mut text = valid_config_text();
    text.push_str("\nfiles:\n  - source: relative/file\n    destination: .config/x\n");
    write_config(&location, &text)?;

    let error = load(&location).refused_because("relative sources are refused")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::FileDeclarationInvalidSource)
    );
    Ok(())
}

#[test]
fn declared_file_destinations_must_stay_under_the_sandbox_home() -> Checked {
    use std::fmt::Write as _;

    let (_dir, location) = location()?;
    for destination in ["/etc/passwd", "../outside", "nested/../../outside"] {
        let mut text = valid_config_text();
        let _ = write!(
            text,
            "\nfiles:\n  - source: /tmp/source\n    destination: \"{destination}\"\n"
        );
        write_config(&location, &text)?;

        let error = load(&location).refused_because("{destination} must be refused")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::FileDeclarationInvalidDestination),
            "destination {destination} produced the wrong error"
        );
    }
    Ok(())
}

/// sbxmのvalidationは通るが、YAMLとしては別の型や構造に読めてしまう値。
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
fn rendered_values_survive_a_round_trip_even_when_they_look_like_yaml_syntax() -> Checked {
    for value in yaml_lookalike_values() {
        let config = GlobalConfig {
            language: Some(Locale::En),
            // 名義も利用者が打った任意の文字列である。YAMLの意味を持つ値でも往復する。
            git_identity: Some(GitIdentity {
                user_name: value.to_string(),
                user_email: format!("{value}@example.com"),
            }),
            files: vec![FileDeclaration {
                source: HostFileSource::new(&format!("/hosts/{value}")).required()?,
                destination: SandboxHomeRelativePath::new(&format!(".config/{value}"))
                    .required()?,
            }],
        };

        let rendered = render(&config)?;
        let state = parse(&rendered, Path::new("/tmp/config.yaml")).required_because(&format!(
            "{value:?} rendered YAML that does not parse:\n{rendered}"
        ))?;
        let ConfigState::Valid {
            config: loaded,
            warnings,
        } = state
        else {
            return Err(Unmet::new(format!(
                "{value:?} must render a complete configuration"
            )));
        };
        assert!(warnings.is_empty(), "{value:?} produced {warnings:?}");
        assert_eq!(*loaded, config, "{value:?} did not survive the round trip");
    }
    Ok(())
}

#[test]
fn rendering_quotes_values_that_need_escaping() -> Checked {
    let config = GlobalConfig {
        language: Some(Locale::En),
        git_identity: None,
        files: vec![FileDeclaration {
            source: HostFileSource::new("/Users/ex ample/.gitconfig").required()?,
            destination: SandboxHomeRelativePath::new(".gitconfig").required()?,
        }],
    };
    let rendered = render(&config)?;
    let reparsed: yaml_serde::Value =
        yaml_serde::from_str(&rendered).required_because("rendered config is YAML")?;
    assert_eq!(
        reparsed["files"][0]["source"].as_str(),
        Some("/Users/ex ample/.gitconfig")
    );
    Ok(())
}

#[test]
fn a_configuration_that_cannot_be_edited_one_line_at_a_time_is_left_alone() -> Checked {
    let (_dir, location) = location()?;
    // 有効だが行指向ではない書き方。行単位の編集では安全に足せない。
    write_config(&location, "{version: 1}\n")?;

    let error = save_language(&location, Locale::Ja)
        .refused_because("a configuration sbxm cannot edit is never rewritten")?;
    assert_eq!(error.first_id(), Some(ErrorId::ConfigNotRewritable));
    assert_eq!(
        fs::read_to_string(location.config_file()).required()?,
        "{version: 1}\n",
        "the user's configuration is untouched"
    );
    // 拒否したあとも、そのconfigはそのまま読める。
    assert_eq!(loaded(&location)?.language, None);
    Ok(())
}

// --- 名義 ---

fn example_identity() -> GitIdentity {
    GitIdentity {
        user_name: "Example User".to_string(),
        user_email: "user@example.com".to_string(),
    }
}

#[test]
fn the_git_identity_is_saved_without_rewriting_what_the_user_wrote() -> Checked {
    let (_dir, location) = location()?;
    let handwritten = "\
# my settings
version: 1
language: ja

files:
  - source: /Users/example/.gitconfig
    destination: .gitconfig
";
    write_config(&location, handwritten)?;

    let path = save_git_identity(&location, &example_identity())
        .required_because("the identity is saved")?;
    let written = fs::read_to_string(&path).required()?;
    assert_eq!(
        written,
        "\
# my settings
version: 1
git_user_name: Example User
git_user_email: user@example.com
language: ja

files:
  - source: /Users/example/.gitconfig
    destination: .gitconfig
",
        "comments, blank lines, key order and files are kept"
    );
    assert_eq!(loaded(&location)?.git_identity, Some(example_identity()));
    assert_eq!(
        loaded(&location)?.language,
        Some(Locale::Ja),
        "saving one setting leaves the others alone"
    );

    // 二度目は2行を差し替えるだけで、重複させない。
    let replaced = GitIdentity {
        user_name: "Other User".to_string(),
        user_email: "other@example.com".to_string(),
    };
    save_git_identity(&location, &replaced).required_because("the identity is replaced")?;
    let written = fs::read_to_string(&path).required()?;
    assert_eq!(loaded(&location)?.git_identity, Some(replaced));
    assert_eq!(
        written.matches("git_user_name:").count(),
        1,
        "the line is replaced rather than repeated: {written}"
    );
    assert!(!written.contains("Example User"), "{written}");
    assert!(written.starts_with("# my settings\n"), "{written}");
    Ok(())
}

#[test]
fn saving_the_identity_creates_a_private_configuration_when_there_is_none() -> Checked {
    let (_dir, location) = location()?;
    let path = save_git_identity(&location, &example_identity())
        .required_because("the configuration is created")?;

    let file_mode = fs::metadata(&path).required()?.permissions().mode() & 0o777;
    assert_eq!(file_mode, 0o600);
    assert_eq!(loaded(&location)?.git_identity, Some(example_identity()));
    assert_eq!(
        loaded(&location)?.language,
        None,
        "choosing an identity does not choose a language"
    );
    Ok(())
}

#[test]
fn an_unsaved_identity_is_absent_rather_than_written_as_null() -> Checked {
    let config = GlobalConfig {
        language: Some(Locale::En),
        git_identity: None,
        files: Vec::new(),
    };
    let rendered = render(&config)?;
    assert!(
        !rendered.contains("git_user_name"),
        "not having chosen is not the same as having chosen nothing: {rendered}"
    );
    Ok(())
}

#[test]
fn half_a_declared_identity_is_refused_rather_than_half_applied() -> Checked {
    // 名義は2つで1つの意図である。残りを推測して補わず、欠けているfieldを名指す。
    for (text, missing) in [
        (
            "version: 1\ngit_user_name: Example User\n",
            "git_user_email",
        ),
        (
            "version: 1\ngit_user_email: user@example.com\n",
            "git_user_name",
        ),
    ] {
        let (_dir, location) = location()?;
        write_config(&location, text)?;

        let error = load(&location).refused_because("{text} must be refused")?;
        assert_eq!(error.first_id(), Some(ErrorId::ConfigMissingField));
        let described = &error.diagnostics()[0].description;
        assert!(
            described
                .args
                .iter()
                .any(|(key, value)| *key == "field" && value == missing),
            "the error names the field that is missing: {:?}",
            described.args
        );
    }
    Ok(())
}

#[test]
fn an_identity_value_that_git_cannot_use_is_refused() -> Checked {
    for text in [
        "version: 1\ngit_user_name: \"\"\ngit_user_email: user@example.com\n",
        "version: 1\ngit_user_name: Example User\ngit_user_email: \"   \"\n",
    ] {
        let (_dir, location) = location()?;
        write_config(&location, text)?;

        let error = load(&location).refused_because("{text} must be refused")?;
        assert_eq!(error.first_id(), Some(ErrorId::ConfigInvalidValue));
    }
    Ok(())
}
