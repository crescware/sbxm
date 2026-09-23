use crate::diagnostics::ErrorId;
use crate::paths::{PRIVATE_DIR_MODE, PRIVATE_FILE_MODE};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use crate::testing::outcome::{Checked, Refused, Required, Unmet};

use super::*;

fn location() -> Checked<(tempfile::TempDir, ConfigLocation)> {
    let dir = tempfile::tempdir().required_because("temporary home")?;
    let location = ConfigLocation::from_home(dir.path().to_path_buf());
    Ok((dir, location))
}

fn write_config(location: &ConfigLocation, text: &str) -> Checked {
    let dir = location.dir();
    fs::create_dir_all(&dir).required()?;
    fs::set_permissions(&dir, fs::Permissions::from_mode(PRIVATE_DIR_MODE)).required()?;
    let path = location.config_file();
    fs::write(&path, text).required()?;
    fs::set_permissions(&path, fs::Permissions::from_mode(PRIVATE_FILE_MODE)).required()?;
    Ok(())
}

fn written(location: &ConfigLocation) -> Checked<String> {
    fs::read_to_string(location.config_file()).required_because("read the configuration")
}

fn loaded(location: &ConfigLocation) -> Checked<GlobalConfig> {
    let ConfigState::Valid { config, .. } = load(location).required()? else {
        return Err(Unmet::new("the configuration must be present".to_string()));
    };
    Ok(*config)
}

fn declared(source: &str, destination: &str) -> Checked<FileDeclaration> {
    Ok(FileDeclaration {
        source: HostFileSource::new(source).required()?,
        destination: SandboxHomeRelativePath::new(destination).required()?,
    })
}

fn claude() -> Checked<FileDeclaration> {
    declared("/Users/example/.claude/CLAUDE.md", ".claude/CLAUDE.md")
}

#[test]
fn a_declaration_creates_a_private_configuration_when_there_is_none() -> Checked {
    let (_dir, location) = location()?;
    let path = save_file_declaration(&location, &claude()?).required()?;

    assert_eq!(
        fs::metadata(&path).required()?.permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(loaded(&location)?.files, vec![claude()?]);
    Ok(())
}

#[test]
fn a_declaration_is_added_in_the_style_the_configuration_already_uses() -> Checked {
    // (原文, 足したあとの原文)。既存の宣言があればその`-`の書き方と字下げに揃え、
    // 宣言が無ければsbxmの文書と同じ字下げで足す。ほかの記述は1文字も変わらない。
    let cases = [
        (
            "# my settings\nversion: 1\nlanguage: ja\n",
            "# my settings\nversion: 1\nlanguage: ja\nfiles:\n  - source: /Users/example/.claude/CLAUDE.md\n    destination: .claude/CLAUDE.md\n",
        ),
        (
            "version: 1",
            "version: 1\nfiles:\n  - source: /Users/example/.claude/CLAUDE.md\n    destination: .claude/CLAUDE.md\n",
        ),
        (
            "version: 1\n\nfiles:\n  - source: /Users/example/.gitconfig\n    destination: .gitconfig # keep\n  # later\nlanguage: ja\n",
            "version: 1\n\nfiles:\n  - source: /Users/example/.gitconfig\n    destination: .gitconfig # keep\n  - source: /Users/example/.claude/CLAUDE.md\n    destination: .claude/CLAUDE.md\n  # later\nlanguage: ja\n",
        ),
        (
            "version: 1\nfiles:\n- source: /Users/example/.gitconfig\n  destination: .gitconfig\n",
            "version: 1\nfiles:\n- source: /Users/example/.gitconfig\n  destination: .gitconfig\n- source: /Users/example/.claude/CLAUDE.md\n  destination: .claude/CLAUDE.md\n",
        ),
        (
            "version: 1\nfiles:\n    -   source: /Users/example/.gitconfig\n        destination: .gitconfig\n",
            "version: 1\nfiles:\n    -   source: /Users/example/.gitconfig\n        destination: .gitconfig\n    -   source: /Users/example/.claude/CLAUDE.md\n        destination: .claude/CLAUDE.md\n",
        ),
        (
            "version: 1\nfiles:\n  - {source: /Users/example/.gitconfig, destination: .gitconfig}\n",
            "version: 1\nfiles:\n  - {source: /Users/example/.gitconfig, destination: .gitconfig}\n  - source: /Users/example/.claude/CLAUDE.md\n    destination: .claude/CLAUDE.md\n",
        ),
        (
            "version: 1\nfiles: []\n",
            "version: 1\nfiles: [{source: \"/Users/example/.claude/CLAUDE.md\", destination: \".claude/CLAUDE.md\"}]\n",
        ),
        (
            "version: 1\nfiles: [{source: /Users/example/.gitconfig, destination: .gitconfig}]\n",
            "version: 1\nfiles: [{source: /Users/example/.gitconfig, destination: .gitconfig}, {source: \"/Users/example/.claude/CLAUDE.md\", destination: \".claude/CLAUDE.md\"}]\n",
        ),
        (
            "version: 1\nfiles: # nothing yet\nlanguage: ja\n",
            "version: 1\nfiles: # nothing yet\n  - source: /Users/example/.claude/CLAUDE.md\n    destination: .claude/CLAUDE.md\nlanguage: ja\n",
        ),
    ];
    for (before, after) in cases {
        let (_dir, location) = location()?;
        write_config(&location, before)?;
        let existing = loaded(&location)
            .map(|config| config.files)
            .unwrap_or_default();

        save_file_declaration(&location, &claude()?)
            .required_because(&format!("{before:?} takes a declaration"))?;

        assert_eq!(written(&location)?, after, "from {before:?}");
        let mut expected = existing;
        expected.push(claude()?);
        assert_eq!(loaded(&location)?.files, expected, "from {before:?}");
    }
    Ok(())
}

#[test]
fn declared_paths_that_look_like_yaml_syntax_survive_the_edit() -> Checked {
    for value in [
        "no",
        "#hash",
        "a: b",
        "- item",
        "'single'",
        "  padded",
        "日本語 🙂",
    ] {
        for before in ["version: 1\n", "version: 1\nfiles: []\n"] {
            let (_dir, location) = location()?;
            write_config(&location, before)?;
            let file = declared(&format!("/hosts/{value}"), &format!(".config/{value}"))?;

            save_file_declaration(&location, &file)
                .required_because(&format!("{value:?} is declared into {before:?}"))?;
            assert_eq!(loaded(&location)?.files, vec![file], "{value:?} {before:?}");
        }
    }
    Ok(())
}

#[test]
fn a_declaration_that_cannot_be_added_safely_leaves_the_configuration_alone() -> Checked {
    let (_dir, location) = location()?;
    // top-levelがflow styleのconfig。宣言をどう書き足すかを決めない。
    write_config(&location, "{version: 1}\n")?;

    let error = save_file_declaration(&location, &claude()?)
        .refused_because("sbxm does not guess how to extend this style")?;
    let diagnostic = error.diagnostics().first().required()?;
    assert_eq!(diagnostic.id, ErrorId::ConfigNotRewritable);
    // 書けないと分かった以上、利用者が手で書き足せる宣言をそのまま渡す。
    let declaration = diagnostic
        .remediation
        .as_ref()
        .and_then(|remediation| remediation.explanation.first())
        .and_then(|message| {
            message
                .args
                .iter()
                .find_map(|(key, value)| (*key == "declaration").then(|| value.clone()))
        })
        .required_because("the lines to add by hand")?;
    assert_eq!(
        declaration,
        "files:\n  - source: /Users/example/.claude/CLAUDE.md\n    destination: .claude/CLAUDE.md"
    );
    assert_eq!(written(&location)?, "{version: 1}\n");
    Ok(())
}

#[test]
fn a_configuration_that_does_not_load_is_refused_for_its_own_reason() -> Checked {
    let (_dir, location) = location()?;
    write_config(&location, "version: 99\n")?;

    let error = save_file_declaration(&location, &claude()?)
        .refused_because("an unknown version is not extended")?;
    assert_eq!(error.first_id(), Some(ErrorId::ConfigUnknownVersion));
    assert_eq!(written(&location)?, "version: 99\n");
    Ok(())
}

#[test]
fn a_declaration_is_removed_by_its_destination() -> Checked {
    let (_dir, location) = location()?;
    write_config(
        &location,
        "version: 1\nfiles:\n  - source: /Users/example/.gitconfig\n    destination: .gitconfig\n  - source: /Users/example/.claude/CLAUDE.md\n    destination: .claude/CLAUDE.md\nlanguage: ja\n",
    )?;

    let (_path, removed) = remove_file_declaration(
        &location,
        &SandboxHomeRelativePath::new("./.claude/CLAUDE.md").required()?,
    )
    .required_because("the spelling with a leading ./ names the same destination")?;

    assert_eq!(removed, claude()?);
    assert_eq!(
        written(&location)?,
        "version: 1\nfiles:\n  - source: /Users/example/.gitconfig\n    destination: .gitconfig\nlanguage: ja\n"
    );

    // 最後の1件を外すと、`files`は空のlistとして残る。
    remove_file_declaration(
        &location,
        &SandboxHomeRelativePath::new(".gitconfig").required()?,
    )
    .required()?;
    assert_eq!(written(&location)?, "version: 1\nfiles: []\nlanguage: ja\n");
    assert!(loaded(&location)?.files.is_empty());
    Ok(())
}

#[test]
fn removing_a_destination_that_is_not_declared_is_refused() -> Checked {
    let destination = SandboxHomeRelativePath::new(".claude/CLAUDE.md").required()?;

    // configが無い。
    let (_empty, nothing) = location()?;
    let error =
        remove_file_declaration(&nothing, &destination).refused_because("nothing is declared")?;
    assert_eq!(error.first_id(), Some(ErrorId::FileNotDeclared));

    // 別の配置先だけが宣言されている。
    let (_dir, location) = location()?;
    let text =
        "version: 1\nfiles:\n  - source: /Users/example/.gitconfig\n    destination: .gitconfig\n";
    write_config(&location, text)?;
    let error = remove_file_declaration(&location, &destination)
        .refused_because("the destination is not declared")?;
    assert_eq!(error.first_id(), Some(ErrorId::FileNotDeclared));
    assert_eq!(written(&location)?, text);
    Ok(())
}

#[test]
fn an_unknown_key_in_a_declared_file_is_a_warning() -> Checked {
    // 後のversionが宣言fileへ足す項目を、このbuildは解さないまま配置することになる。
    let path = Path::new("/Users/example/.sbxm/config.yaml");
    let state = parse(
        "version: 1\nfiles:\n  - source: /a\n    destination: a\n    sync: both\n",
        path,
    )
    .required()?;
    let ConfigState::Valid { warnings, .. } = state else {
        return Err(Unmet::new("the configuration loads".to_string()));
    };
    assert_eq!(warnings.len(), 1);
    assert_eq!(
        warnings[0].description.id,
        "warning-config-unknown-file-key"
    );
    let key = warnings[0]
        .description
        .args
        .iter()
        .find_map(|(name, value)| (*name == "key").then(|| value.clone()))
        .required()?;
    assert_eq!(key, "sync");
    Ok(())
}

#[test]
fn a_declaration_that_cannot_be_removed_safely_leaves_the_configuration_alone() -> Checked {
    let (_dir, location) = location()?;
    // 外す宣言のanchorを、別のkeyが参照している。外せばそのkeyも壊れる。
    let text = "version: 1\nfiles:\n  - &shared\n    source: /Users/example/.gitconfig\n    destination: .gitconfig\nfuture_option: *shared\n";
    write_config(&location, text)?;

    let error = remove_file_declaration(
        &location,
        &SandboxHomeRelativePath::new(".gitconfig").required()?,
    )
    .refused_because("another key would lose what it refers to")?;
    let diagnostic = error.diagnostics().first().required()?;
    assert_eq!(diagnostic.id, ErrorId::ConfigNotRewritable);
    assert_eq!(
        diagnostic
            .remediation
            .as_ref()
            .and_then(|remediation| remediation.explanation.first())
            .map(|message| message.id),
        Some("remediation-file-declaration-not-removable")
    );
    assert_eq!(written(&location)?, text);
    Ok(())
}

#[test]
fn removing_from_a_configuration_that_does_not_load_is_refused_for_its_own_reason() -> Checked {
    let (_dir, location) = location()?;
    write_config(&location, "version: 99\n")?;

    let error = remove_file_declaration(
        &location,
        &SandboxHomeRelativePath::new(".gitconfig").required()?,
    )
    .refused_because("an unknown version is not edited")?;
    assert_eq!(error.first_id(), Some(ErrorId::ConfigUnknownVersion));
    Ok(())
}
