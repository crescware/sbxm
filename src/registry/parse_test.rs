use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::testing::registry::{Entry, document};

fn parsed(text: &str) -> Checked<Registry> {
    parse(text, Path::new("/home/user/.sbxm/registry.yaml")).required_because("the document parses")
}

fn refused(text: &str) -> Checked<Error> {
    parse(text, Path::new("/home/user/.sbxm/registry.yaml"))
        .refused_because("the document is refused")
}

#[test]
fn a_document_without_entries_holds_no_project() -> Checked {
    for text in ["version: 1\n", "version: 1\nprojects: []\n"] {
        assert!(
            parsed(text)?.entries().is_empty(),
            "{text:?} must hold no project"
        );
    }
    Ok(())
}

#[test]
fn an_entry_is_read_back_as_the_repository_it_names() -> Checked {
    let registry = parsed(&document(&[Entry::example()]))?;
    let entries = registry.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].canonical_id().as_str(), "example-org/alpha");
    assert_eq!(
        entries[0].project_root(),
        Path::new("/home/user/Projects/alpha.project")
    );
    assert_eq!(
        entries[0].repository().clone_url(),
        "git@github.com:Example-Org/Alpha.git"
    );
    // 表示上の綴りはclone URLから読み直す。索引は二重に保存しない。
    assert_eq!(entries[0].repository().display_id(), "Example-Org/Alpha");
    Ok(())
}

#[test]
fn entries_come_back_in_canonical_order_whatever_order_they_were_written_in() -> Checked {
    let registry = parsed(&document(&[
        Entry::of("zeta/zulu", "/home/user/Projects/zulu.project", "ssh"),
        Entry::of("alpha/alfa", "/home/user/Projects/alfa.project", "https"),
    ]))?;
    let ids: Vec<&str> = registry
        .entries()
        .iter()
        .map(|entry| entry.canonical_id().as_str())
        .collect();
    assert_eq!(ids, vec!["alpha/alfa", "zeta/zulu"]);
    Ok(())
}

#[test]
fn an_unknown_version_is_diagnosed_before_the_entries() -> Checked {
    let error = refused("version: 2\nprojects: []\n")?;
    assert_eq!(error.first_id(), Some(ErrorId::RegistryUnknownVersion));

    let error = refused("projects: []\n")?;
    assert_eq!(error.first_id(), Some(ErrorId::RegistryMissingField));
    Ok(())
}

#[test]
fn required_fields_are_named_when_they_are_missing() -> Checked {
    for field in [
        "canonical_id",
        "project_root",
        "provider",
        "clone_transport",
        "clone_url",
    ] {
        let text = document(&[Entry::example().without(field)]);
        let error = refused(&text)?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::RegistryMissingField),
            "{field} produced the wrong error"
        );
    }
    Ok(())
}

#[test]
fn a_field_that_disagrees_with_the_clone_url_is_refused() -> Checked {
    for (field, value) in [
        ("canonical_id", "other-org/alpha"),
        ("clone_transport", "https"),
        ("provider", "gitlab"),
        ("clone_url", "git@github.com:Example-Org/Alpha"),
    ] {
        let text = document(&[Entry::example().with(field, value)]);
        let error = refused(&text)?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::RegistryInvalidValue),
            "{field}: {value} produced the wrong error"
        );
    }
    Ok(())
}

#[test]
fn a_project_root_that_is_not_an_absolute_path_is_refused() -> Checked {
    for root in ["Projects/alpha.project", "/home/user/../etc/alpha.project"] {
        let text = document(&[Entry::example().with("project_root", root)]);
        assert_eq!(
            refused(&text)?.first_id(),
            Some(ErrorId::RegistryInvalidValue),
            "{root} produced the wrong error"
        );
    }
    Ok(())
}

#[test]
fn a_field_the_document_never_defines_is_refused_rather_than_ignored() -> Checked {
    // 観測から算出できる状態をregistryへ保存しない。書かれていたfieldを黙って
    // 無視すると、その不変条件を守れない。
    let text = document(&[Entry::example()]).replace(
        "  provider: github\n",
        "  provider: github\n  state: registered\n",
    );
    assert_eq!(
        refused(&text)?.first_id(),
        Some(ErrorId::RegistryInvalidSyntax)
    );

    let text = format!("{}sandboxes: []\n", document(&[Entry::example()]));
    assert_eq!(
        refused(&text)?.first_id(),
        Some(ErrorId::RegistryInvalidSyntax)
    );
    Ok(())
}
