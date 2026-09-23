use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{self, ConfigLocation, ConfigState, GlobalConfig};
use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused, Required, Unmet};

use super::*;

/// home directoryと、その下に置いた宣言候補のfile。
fn home_with(
    relative: &str,
    contents: &[u8],
) -> Checked<(tempfile::TempDir, ConfigLocation, PathBuf)> {
    let home = tempfile::tempdir().required()?;
    let location = ConfigLocation::from_home(home.path().to_path_buf());
    let file = home.path().join(relative);
    fs::create_dir_all(file.parent().required()?).required()?;
    fs::write(&file, contents).required()?;
    let file = absolute_source(&file).required()?;
    Ok((home, location, file))
}

fn loaded(location: &ConfigLocation) -> Checked<GlobalConfig> {
    let ConfigState::Valid { config, .. } = config::load(location).required()? else {
        return Err(Unmet::new("the configuration is saved".to_string()));
    };
    Ok(*config)
}

#[test]
fn a_file_under_home_is_placed_at_the_same_path_under_the_sandbox_home() -> Checked {
    let (_home, location, file) = home_with(".claude/CLAUDE.md", b"# notes\n")?;

    let added = add(&location, &GlobalConfig::default(), &file, None).required()?;

    assert!(!added.already);
    assert_eq!(
        crate::paths::display(added.declaration.destination.as_path()),
        ".claude/CLAUDE.md"
    );
    assert_eq!(added.path, location.config_file());
    assert_eq!(loaded(&location)?.files, vec![added.declaration]);
    Ok(())
}

#[test]
fn an_explicit_destination_is_recorded_without_its_leading_dot_segment() -> Checked {
    let (_home, location, file) = home_with("notes.md", b"notes\n")?;

    let added = add(
        &location,
        &GlobalConfig::default(),
        &file,
        Some("./.config/notes.md"),
    )
    .required()?;
    assert_eq!(
        crate::paths::display(added.declaration.destination.as_path()),
        ".config/notes.md"
    );
    Ok(())
}

#[test]
fn a_destination_that_leaves_the_sandbox_home_is_refused_before_anything_is_saved() -> Checked {
    let (_home, location, file) = home_with("notes.md", b"notes\n")?;
    for destination in ["/etc/notes.md", "../notes.md", ".", ""] {
        let error = add(
            &location,
            &GlobalConfig::default(),
            &file,
            Some(destination),
        )
        .refused_because(&format!("{destination:?} is refused"))?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::FileDeclarationInvalidDestination),
            "{destination:?}"
        );
    }
    assert!(!location.config_file().exists(), "nothing is saved");
    Ok(())
}

#[test]
fn a_file_outside_home_needs_an_explicit_destination() -> Checked {
    let (_home, location, _) = home_with("placeholder", b"")?;
    let elsewhere = tempfile::tempdir().required()?;
    let outside = elsewhere.path().join("notes.md");
    fs::write(&outside, b"notes\n").required()?;
    let outside = absolute_source(&outside).required()?;

    let error = add(&location, &GlobalConfig::default(), &outside, None)
        .refused_because("there is no home-relative path to reuse")?;
    assert_eq!(error.first_id(), Some(ErrorId::FileDestinationRequired));

    add(
        &location,
        &GlobalConfig::default(),
        &outside,
        Some(".config/notes.md"),
    )
    .required_because("an explicit destination is enough")?;
    Ok(())
}

#[test]
fn a_source_that_could_not_be_placed_is_refused_when_it_is_declared() -> Checked {
    let (home, location, _) = home_with("placeholder", b"")?;
    let directory = home.path().join(".claude");
    fs::create_dir_all(&directory).required()?;
    let large = home.path().join("large.bin");
    fs::write(&large, vec![0_u8; 1024 * 1024 + 1]).required()?;
    let link = home.path().join("link.md");
    std::os::unix::fs::symlink(home.path().join("placeholder"), &link).required()?;

    for source in [directory, large, link, home.path().join("absent.md")] {
        let error = add(&location, &GlobalConfig::default(), &source, None).refused_because(
            &format!("{} is not a file sbxm can place", source.display()),
        )?;
        assert_eq!(error.first_id(), Some(ErrorId::DeclaredFileUnusable));
    }
    assert!(!location.config_file().exists(), "nothing is saved");
    Ok(())
}

#[test]
fn the_same_declaration_twice_changes_nothing_but_another_source_is_refused() -> Checked {
    let (home, location, file) = home_with(".claude/CLAUDE.md", b"# notes\n")?;
    let first = add(&location, &GlobalConfig::default(), &file, None).required()?;
    let before = fs::read_to_string(location.config_file()).required()?;

    let again = add(&location, &loaded(&location)?, &file, None).required()?;
    assert!(again.already);
    assert_eq!(
        fs::read_to_string(location.config_file()).required()?,
        before
    );

    // 同じ配置先に別のsourceを置くと、どちらを置くかを推測することになる。
    let other = home.path().join("other.md");
    fs::write(&other, b"other\n").required()?;
    let error = add(
        &location,
        &loaded(&location)?,
        &absolute_source(&other).required()?,
        Some("./.claude/CLAUDE.md"),
    )
    .refused_because("the destination already has a source")?;
    assert_eq!(error.first_id(), Some(ErrorId::FileAlreadyDeclared));
    assert_eq!(loaded(&location)?.files, vec![first.declaration]);
    Ok(())
}

#[test]
fn a_name_that_often_holds_credentials_is_flagged_but_still_declared() -> Checked {
    let (_home, location, file) = home_with(".config/tool/token.json", b"{}\n")?;
    let added = add(&location, &GlobalConfig::default(), &file, None).required()?;
    assert!(added.credential_like);
    assert_eq!(loaded(&location)?.files.len(), 1);

    let (_home, location, file) = home_with(".claude/CLAUDE.md", b"# notes\n")?;
    let added = add(&location, &GlobalConfig::default(), &file, None).required()?;
    assert!(!added.credential_like);
    Ok(())
}

#[test]
fn names_that_often_hold_credentials_are_recognized() {
    for name in [
        ".env",
        ".env.local",
        ".netrc",
        ".git-credentials",
        "credentials",
        "credentials.json",
        "client_secret.json",
        "github-token",
        "id_rsa",
        "id_ed25519",
        "server.pem",
        "tls.key",
        "PASSWORD.txt",
    ] {
        assert!(looks_like_credential(name), "{name}");
    }
    for name in [
        "CLAUDE.md",
        ".gitconfig",
        "settings.json",
        "environment.md",
        "keys.md",
    ] {
        assert!(!looks_like_credential(name), "{name}");
    }
}

#[test]
fn a_relative_source_is_resolved_from_the_current_directory() -> Checked {
    let current = std::env::current_dir().required()?;
    assert_eq!(
        absolute_source(Path::new("Cargo.toml")).required()?,
        fs::canonicalize(&current).required()?.join("Cargo.toml")
    );
    // 途中の`..`は実体のdirectoryへ解決する。
    assert_eq!(
        absolute_source(Path::new("src/../Cargo.toml")).required()?,
        fs::canonicalize(&current).required()?.join("Cargo.toml")
    );
    Ok(())
}

#[test]
fn a_source_whose_directory_does_not_exist_is_refused() -> Checked {
    for given in ["/nonexistent-sbxm-directory/file.md", "/"] {
        let error = absolute_source(Path::new(given))
            .refused_because(&format!("{given} cannot be resolved"))?;
        assert_eq!(error.first_id(), Some(ErrorId::DeclaredFileUnusable));
    }
    Ok(())
}

#[test]
fn a_declaration_is_removed_by_its_destination() -> Checked {
    let (_home, location, file) = home_with(".claude/CLAUDE.md", b"# notes\n")?;
    let added = add(&location, &GlobalConfig::default(), &file, None).required()?;

    let (path, removed) = remove(&location, ".claude/CLAUDE.md").required()?;
    assert_eq!(path, location.config_file());
    assert_eq!(removed, added.declaration);
    assert!(loaded(&location)?.files.is_empty());

    let error = remove(&location, "../outside").refused_because("not a sandbox path")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::FileDeclarationInvalidDestination)
    );
    Ok(())
}

#[test]
fn the_question_is_answered_by_which_of_its_two_choices_was_taken() -> Checked {
    use crate::i18n::Locale;
    use crate::testing::prompt::ScriptedPrompt;

    assert!(ask_to_apply(&mut ScriptedPrompt::choosing(0), 2, Locale::En).required()?);
    assert!(!ask_to_apply(&mut ScriptedPrompt::choosing(1), 2, Locale::En).required()?);

    let mut prompt = ScriptedPrompt::choosing(0);
    ask_to_apply(&mut prompt, 3, Locale::Ja).required()?;
    assert_eq!(
        prompt.asked.borrow().first().map(Vec::len),
        Some(2),
        "placing now and later are the only choices"
    );

    // 候補に無い選択はcancelではない。promptの契約違反として区別する。
    let error = ask_to_apply(&mut ScriptedPrompt::choosing(2), 2, Locale::En)
        .refused_because("an index outside the choices is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::SelectionUnresolved));
    Ok(())
}

#[test]
fn control_and_direction_characters_are_shown_rather_than_obeyed() {
    assert_eq!(visible("plain\ttext\n"), "plain\ttext\n");
    assert_eq!(visible("日本語 🙂"), "日本語 🙂");
    assert_eq!(visible("\u{1b}[2Kgone"), "\\u{1b}[2Kgone");
    assert_eq!(
        visible("a\u{202e}b\u{2066}c\u{200f}"),
        "a\\u{202e}b\\u{2066}c\\u{200f}"
    );
    assert_eq!(visible("bell\u{7}\r"), "bell\\u{7}\\u{d}");
}

#[test]
fn the_host_git_shows_how_two_files_differ() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let before = dir.path().join("before.md");
    let after = dir.path().join("after.md");
    fs::write(&before, b"line\nold\n").required()?;
    fs::write(&after, b"line\nnew\n").required()?;
    let host = crate::boundary::host::RealHost;

    let diff = host_diff(&host, &before, &after).required()?;
    assert!(diff.contains("-old") && diff.contains("+new"), "{diff}");

    // 同じ内容には差分が無い。
    assert!(host_diff(&host, &before, &before).required()?.is_empty());

    // gitが比べられなければ、差分が無いとは読まない。
    let error = host_diff(&host, &before, &dir.path().join("absent.md"))
        .refused_because("there is nothing to compare")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));
    Ok(())
}
