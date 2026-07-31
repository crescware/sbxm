use crate::config::ConfigLocation;
use crate::diagnostics::ErrorId;
use crate::paths::{PRIVATE_FILE_MODE, PathScope, acquire_exclusive_lock};
use std::path::Path;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::testing::project::{https_repository, ssh_repository};
use crate::testing::registry::{Entry, document};
use std::os::unix::fs::PermissionsExt;

fn home() -> Checked<(tempfile::TempDir, ConfigLocation)> {
    let dir = tempfile::tempdir().required_because("temporary home")?;
    let location = ConfigLocation::from_home(dir.path().to_path_buf());
    Ok((dir, location))
}

fn write_registry(location: &ConfigLocation, text: &str) -> Checked {
    std::fs::create_dir_all(location.dir()).required_because("create ~/.sbxm")?;
    std::fs::set_permissions(location.dir(), std::fs::Permissions::from_mode(0o700))
        .required_because("mode")?;
    std::fs::write(location.registry_file(), text).required_because("write the registry")?;
    std::fs::set_permissions(
        location.registry_file(),
        std::fs::Permissions::from_mode(0o600),
    )
    .required_because("mode")?;
    Ok(())
}

fn entry_for(project: &str, root: &str) -> Checked<RegistryEntry> {
    RegistryEntry::new(Path::new(root), ssh_repository(project)?).required_because("a valid entry")
}

#[test]
fn a_registry_that_was_never_written_holds_no_project() -> Checked {
    let (dir, location) = home()?;
    assert_eq!(
        load(&location).required_because("no registry is no project")?,
        Index::default()
    );
    // read-onlyの読み取りは何も作らない。
    assert_eq!(std::fs::read_dir(dir.path()).required()?.count(), 0);
    Ok(())
}

#[test]
fn an_entry_is_recorded_and_read_back_through_a_fresh_load() -> Checked {
    let (_dir, location) = home()?;
    let mut guard = RegistryGuard::acquire(&location).required_because("acquire")?;
    guard
        .insert(entry_for(
            "Example-Org/Alpha",
            "/home/user/Projects/alpha.project",
        )?)
        .required_because("record the registration intent")?;
    drop(guard);

    let registry = load(&location).required_because("the document is valid")?;
    let entries = registry.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].canonical_id().as_str(), "example-org/alpha");
    assert_eq!(
        entries[0].project_root(),
        Path::new("/home/user/Projects/alpha.project")
    );

    let mode = std::fs::metadata(location.registry_file())
        .required()?
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, PRIVATE_FILE_MODE);
    Ok(())
}

#[test]
fn the_whole_document_is_rewritten_rather_than_appended_to() -> Checked {
    let (_dir, location) = home()?;
    let mut guard = RegistryGuard::acquire(&location).required_because("acquire")?;
    guard
        .insert(entry_for("zeta/zulu", "/home/user/Projects/zulu.project")?)
        .required_because("record")?;
    guard
        .insert(entry_for("alpha/alfa", "/home/user/Projects/alfa.project")?)
        .required_because("record")?;
    drop(guard);

    // 追記であれば書いた順が残る。document全体を組み直すため、canonical順になる。
    let text = std::fs::read_to_string(location.registry_file()).required()?;
    assert_eq!(text, render(&load(&location).required_because("valid")?)?);
    assert!(text.find("alpha/alfa") < text.find("zeta/zulu"), "{text}");
    Ok(())
}

#[test]
fn recording_the_same_registration_twice_changes_nothing() -> Checked {
    let (_dir, location) = home()?;
    let mut guard = RegistryGuard::acquire(&location).required_because("acquire")?;
    let entry = entry_for("example-org/alpha", "/home/user/Projects/alpha.project")?;
    guard.insert(entry.clone()).required_because("record")?;
    let before = std::fs::read_to_string(location.registry_file()).required()?;
    guard
        .insert(entry)
        .required_because("the same registration is a no-op")?;
    assert_eq!(
        std::fs::read_to_string(location.registry_file()).required()?,
        before
    );
    Ok(())
}

#[test]
fn a_second_registration_of_the_same_project_is_refused_instead_of_replacing_the_first() -> Checked
{
    let (_dir, location) = home()?;
    let mut guard = RegistryGuard::acquire(&location).required_because("acquire")?;
    guard
        .insert(entry_for(
            "example-org/alpha",
            "/home/user/Projects/alpha.project",
        )?)
        .required_because("record")?;

    for other in [
        entry_for("example-org/alpha", "/home/user/Elsewhere/alpha.project")?,
        RegistryEntry::new(
            Path::new("/home/user/Projects/alpha.project"),
            https_repository("example-org/alpha")?,
        )
        .required_because("a valid entry")?,
    ] {
        let error = guard
            .insert(other)
            .refused_because("the stored registration is never rewritten")?;
        assert_eq!(error.first_id(), Some(ErrorId::RegistryEntryMismatch));
    }
    Ok(())
}

#[test]
fn two_projects_cannot_claim_the_same_project_root() -> Checked {
    let (_dir, location) = home()?;
    let mut guard = RegistryGuard::acquire(&location).required_because("acquire")?;
    guard
        .insert(entry_for(
            "example-org/alpha",
            "/home/user/Projects/alpha.project",
        )?)
        .required_because("record")?;
    let error = guard
        .insert(entry_for(
            "other-org/alpha",
            "/home/user/Projects/alpha.project",
        )?)
        .refused_because("one project root belongs to one project")?;
    assert_eq!(error.first_id(), Some(ErrorId::RegistryDuplicateRoot));
    Ok(())
}

#[test]
fn a_project_root_that_is_not_absolute_is_never_recorded() -> Checked {
    for root in ["Projects/alpha.project", "/home/user/../alpha.project"] {
        let error = RegistryEntry::new(Path::new(root), ssh_repository("example-org/alpha")?)
            .refused_because("{root} is not a project root")?;
        assert_eq!(error.first_id(), Some(ErrorId::RegistryInvalidValue));
    }
    Ok(())
}

#[test]
fn an_entry_that_disappeared_is_removed_and_a_missing_one_is_a_no_op() -> Checked {
    let (_dir, location) = home()?;
    let mut guard = RegistryGuard::acquire(&location).required_because("acquire")?;
    let entry = entry_for("example-org/alpha", "/home/user/Projects/alpha.project")?;
    let canonical = entry.canonical_id().clone();
    guard.insert(entry).required_because("record")?;
    guard.remove(&canonical).required_because("unregister")?;
    assert!(guard.registry().entries().is_empty());

    let before = std::fs::read_to_string(location.registry_file()).required()?;
    guard
        .remove(&canonical)
        .required_because("removing twice is a no-op")?;
    assert_eq!(
        std::fs::read_to_string(location.registry_file()).required()?,
        before
    );
    Ok(())
}

#[test]
fn a_registry_that_cannot_be_trusted_stops_every_mutation() -> Checked {
    let (_dir, location) = home()?;
    write_registry(&location, "version: 2\nprojects: []\n")?;

    let error = load(&location).refused_because("an unknown version is never read as empty")?;
    assert_eq!(error.first_id(), Some(ErrorId::RegistryUnknownVersion));
    let error = RegistryGuard::acquire(&location).refused_because("no mutation starts from it")?;
    assert_eq!(error.first_id(), Some(ErrorId::RegistryUnknownVersion));
    Ok(())
}

#[test]
fn a_document_that_breaks_an_invariant_is_refused_at_load() -> Checked {
    let (_dir, location) = home()?;
    write_registry(
        &location,
        &document(&[
            Entry::example(),
            Entry::of(
                "example-org/alpha",
                "/home/user/Elsewhere/alpha.project",
                "ssh",
            ),
        ]),
    )?;
    let error = load(&location).refused_because("one project has one project root")?;
    assert_eq!(
        error.first_id(),
        Some(ErrorId::RegistryDuplicateProject),
        "{error:?}"
    );
    Ok(())
}

#[test]
fn a_registry_other_accounts_can_read_is_refused_rather_than_repaired() -> Checked {
    let (_dir, location) = home()?;
    write_registry(&location, &document(&[Entry::example()]))?;
    std::fs::set_permissions(
        location.registry_file(),
        std::fs::Permissions::from_mode(0o644),
    )
    .required()?;

    let error = load(&location).refused_because("a world-readable registry is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ConfigPermissionTooOpen));
    Ok(())
}

#[test]
fn a_symlinked_registry_is_never_followed() -> Checked {
    let (dir, location) = home()?;
    let real = dir.path().join("elsewhere.yaml");
    std::fs::write(&real, document(&[Entry::example()])).required()?;
    std::fs::create_dir_all(location.dir()).required()?;
    std::fs::set_permissions(location.dir(), std::fs::Permissions::from_mode(0o700)).required()?;
    std::os::unix::fs::symlink(&real, location.registry_file()).required()?;

    let error = load(&location).refused_because("a symlinked registry is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ConfigSymlink));
    Ok(())
}

#[test]
fn a_concurrent_run_waits_for_the_registry_lock() -> Checked {
    let (_dir, location) = home()?;
    let held =
        RegistryGuard::acquire(&location).required_because("the first run holds the lock")?;

    let path = location.registry_lock();
    let error = acquire_exclusive_lock(
        &path,
        std::time::Duration::from_millis(50),
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
    )
    .refused_because("the second run cannot mutate the registry at the same time")?;
    assert_eq!(error.first_id(), Some(ErrorId::LockTimeout));

    drop(held);
    acquire_exclusive_lock(
        &path,
        std::time::Duration::from_millis(50),
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
    )
    .required_because("the lock is released when the guard ends")?;
    Ok(())
}

#[test]
fn concurrent_registrations_of_different_projects_lose_no_entry() -> Checked {
    let (_dir, location) = home()?;
    let projects = [
        "alpha/alfa",
        "bravo/bravo",
        "charlie/charlie",
        "delta/delta",
    ];

    // 各threadの成立確認は、joinしたあとに呼び出し元で判定する。
    let outcomes = std::thread::scope(|scope| {
        let handles: Vec<_> = projects
            .into_iter()
            .map(|project| {
                let location = location.clone();
                scope.spawn(move || -> Checked {
                    let mut guard = RegistryGuard::acquire(&location)
                        .required_because("the lock serialises the runs")?;
                    let name = project
                        .split('/')
                        .nth(1)
                        .required_because("a repository name")?;
                    let entry = entry_for(project, &format!("/home/user/Projects/{name}.project"))?;
                    guard.insert(entry).required_because("record")?;
                    Ok(())
                })
            })
            .collect();
        handles
            .into_iter()
            .map(std::thread::ScopedJoinHandle::join)
            .collect::<Vec<_>>()
    });
    for outcome in outcomes {
        outcome.required_because("the thread finishes")??;
    }

    let registry = load(&location).required_because("the document stays valid")?;
    let mut recorded: Vec<&str> = registry
        .entries()
        .iter()
        .map(|entry| entry.canonical_id().as_str())
        .collect();
    recorded.sort_unstable();
    assert_eq!(recorded, projects);
    Ok(())
}
