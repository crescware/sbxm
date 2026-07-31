use super::*;
use crate::testing::project::{https_repository, ssh_repository};
use crate::testing::registry::{Entry, document};
use std::os::unix::fs::PermissionsExt;

fn home() -> (tempfile::TempDir, ConfigLocation) {
    let dir = tempfile::tempdir().expect("temporary home");
    let location = ConfigLocation::from_home(dir.path().to_path_buf());
    (dir, location)
}

fn write_registry(location: &ConfigLocation, text: &str) {
    std::fs::create_dir_all(location.dir()).expect("create ~/.sbxm");
    std::fs::set_permissions(location.dir(), std::fs::Permissions::from_mode(0o700)).expect("mode");
    std::fs::write(location.registry_file(), text).expect("write the registry");
    std::fs::set_permissions(
        location.registry_file(),
        std::fs::Permissions::from_mode(0o600),
    )
    .expect("mode");
}

fn entry_for(project: &str, root: &str) -> RegistryEntry {
    RegistryEntry::new(Path::new(root), ssh_repository(project)).expect("a valid entry")
}

#[test]
fn a_registry_that_was_never_written_holds_no_project() {
    let (dir, location) = home();
    assert_eq!(
        load(&location).expect("no registry is no project"),
        Registry::default()
    );
    // read-onlyの読み取りは何も作らない。
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}

#[test]
fn an_entry_is_recorded_and_read_back_through_a_fresh_load() {
    let (_dir, location) = home();
    let mut guard = RegistryGuard::acquire(&location).expect("acquire");
    guard
        .insert(entry_for(
            "Example-Org/Alpha",
            "/home/user/Projects/alpha.project",
        ))
        .expect("record the registration intent");
    drop(guard);

    let registry = load(&location).expect("the document is valid");
    let entries = registry.entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].canonical_id().as_str(), "example-org/alpha");
    assert_eq!(
        entries[0].project_root(),
        Path::new("/home/user/Projects/alpha.project")
    );

    let mode = std::fs::metadata(location.registry_file())
        .unwrap()
        .permissions()
        .mode()
        & 0o777;
    assert_eq!(mode, PRIVATE_FILE_MODE);
}

#[test]
fn the_whole_document_is_rewritten_rather_than_appended_to() {
    let (_dir, location) = home();
    let mut guard = RegistryGuard::acquire(&location).expect("acquire");
    guard
        .insert(entry_for("zeta/zulu", "/home/user/Projects/zulu.project"))
        .expect("record");
    guard
        .insert(entry_for("alpha/alfa", "/home/user/Projects/alfa.project"))
        .expect("record");
    drop(guard);

    // 追記であれば書いた順が残る。document全体を組み直すため、canonical順になる。
    let text = std::fs::read_to_string(location.registry_file()).unwrap();
    assert_eq!(text, render(&load(&location).expect("valid")));
    assert!(text.find("alpha/alfa") < text.find("zeta/zulu"), "{text}");
}

#[test]
fn recording_the_same_registration_twice_changes_nothing() {
    let (_dir, location) = home();
    let mut guard = RegistryGuard::acquire(&location).expect("acquire");
    let entry = entry_for("example-org/alpha", "/home/user/Projects/alpha.project");
    guard.insert(entry.clone()).expect("record");
    let before = std::fs::read_to_string(location.registry_file()).unwrap();
    guard
        .insert(entry)
        .expect("the same registration is a no-op");
    assert_eq!(
        std::fs::read_to_string(location.registry_file()).unwrap(),
        before
    );
}

#[test]
fn a_second_registration_of_the_same_project_is_refused_instead_of_replacing_the_first() {
    let (_dir, location) = home();
    let mut guard = RegistryGuard::acquire(&location).expect("acquire");
    guard
        .insert(entry_for(
            "example-org/alpha",
            "/home/user/Projects/alpha.project",
        ))
        .expect("record");

    for other in [
        entry_for("example-org/alpha", "/home/user/Elsewhere/alpha.project"),
        RegistryEntry::new(
            Path::new("/home/user/Projects/alpha.project"),
            https_repository("example-org/alpha"),
        )
        .expect("a valid entry"),
    ] {
        let error = guard
            .insert(other)
            .expect_err("the stored registration is never rewritten");
        assert_eq!(error.first_id(), Some(ErrorId::RegistryEntryMismatch));
    }
}

#[test]
fn two_projects_cannot_claim_the_same_project_root() {
    let (_dir, location) = home();
    let mut guard = RegistryGuard::acquire(&location).expect("acquire");
    guard
        .insert(entry_for(
            "example-org/alpha",
            "/home/user/Projects/alpha.project",
        ))
        .expect("record");
    let error = guard
        .insert(entry_for(
            "other-org/alpha",
            "/home/user/Projects/alpha.project",
        ))
        .expect_err("one project root belongs to one project");
    assert_eq!(error.first_id(), Some(ErrorId::RegistryDuplicateRoot));
}

#[test]
fn a_project_root_that_is_not_absolute_is_never_recorded() {
    for root in ["Projects/alpha.project", "/home/user/../alpha.project"] {
        let error = RegistryEntry::new(Path::new(root), ssh_repository("example-org/alpha"))
            .expect_err("{root} is not a project root");
        assert_eq!(error.first_id(), Some(ErrorId::RegistryInvalidValue));
    }
}

#[test]
fn an_entry_that_disappeared_is_removed_and_a_missing_one_is_a_no_op() {
    let (_dir, location) = home();
    let mut guard = RegistryGuard::acquire(&location).expect("acquire");
    let entry = entry_for("example-org/alpha", "/home/user/Projects/alpha.project");
    let canonical = entry.canonical_id().clone();
    guard.insert(entry).expect("record");
    guard.remove(&canonical).expect("unregister");
    assert!(guard.registry().entries().is_empty());

    let before = std::fs::read_to_string(location.registry_file()).unwrap();
    guard.remove(&canonical).expect("removing twice is a no-op");
    assert_eq!(
        std::fs::read_to_string(location.registry_file()).unwrap(),
        before
    );
}

#[test]
fn a_registry_that_cannot_be_trusted_stops_every_mutation() {
    let (_dir, location) = home();
    write_registry(&location, "version: 2\nprojects: []\n");

    let error = load(&location).expect_err("an unknown version is never read as empty");
    assert_eq!(error.first_id(), Some(ErrorId::RegistryUnknownVersion));
    let error = RegistryGuard::acquire(&location).expect_err("no mutation starts from it");
    assert_eq!(error.first_id(), Some(ErrorId::RegistryUnknownVersion));
}

#[test]
fn a_document_that_breaks_an_invariant_is_refused_at_load() {
    let (_dir, location) = home();
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
    );
    let error = load(&location).expect_err("one project has one project root");
    assert_eq!(
        error.first_id(),
        Some(ErrorId::RegistryDuplicateProject),
        "{error:?}"
    );
}

#[test]
fn a_registry_other_accounts_can_read_is_refused_rather_than_repaired() {
    let (_dir, location) = home();
    write_registry(&location, &document(&[Entry::example()]));
    std::fs::set_permissions(
        location.registry_file(),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();

    let error = load(&location).expect_err("a world-readable registry is refused");
    assert_eq!(error.first_id(), Some(ErrorId::ConfigPermissionTooOpen));
}

#[test]
fn a_symlinked_registry_is_never_followed() {
    let (dir, location) = home();
    let real = dir.path().join("elsewhere.yaml");
    std::fs::write(&real, document(&[Entry::example()])).unwrap();
    std::fs::create_dir_all(location.dir()).unwrap();
    std::fs::set_permissions(location.dir(), std::fs::Permissions::from_mode(0o700)).unwrap();
    std::os::unix::fs::symlink(&real, location.registry_file()).unwrap();

    let error = load(&location).expect_err("a symlinked registry is refused");
    assert_eq!(error.first_id(), Some(ErrorId::ConfigSymlink));
}

#[test]
fn a_concurrent_run_waits_for_the_registry_lock() {
    let (_dir, location) = home();
    let held = RegistryGuard::acquire(&location).expect("the first run holds the lock");

    let path = location.registry_lock();
    let error = acquire_exclusive_lock(
        &path,
        std::time::Duration::from_millis(50),
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
    )
    .expect_err("the second run cannot mutate the registry at the same time");
    assert_eq!(error.first_id(), Some(ErrorId::LockTimeout));

    drop(held);
    acquire_exclusive_lock(
        &path,
        std::time::Duration::from_millis(50),
        PRIVATE_FILE_MODE,
        PathScope::ConfigFile,
    )
    .expect("the lock is released when the guard ends");
}

#[test]
fn concurrent_registrations_of_different_projects_lose_no_entry() {
    let (_dir, location) = home();
    let projects = [
        "alpha/alfa",
        "bravo/bravo",
        "charlie/charlie",
        "delta/delta",
    ];

    std::thread::scope(|scope| {
        for project in projects {
            let location = location.clone();
            scope.spawn(move || {
                let mut guard =
                    RegistryGuard::acquire(&location).expect("the lock serialises the runs");
                guard
                    .insert(entry_for(
                        project,
                        &format!(
                            "/home/user/Projects/{}.project",
                            project.split('/').nth(1).unwrap()
                        ),
                    ))
                    .expect("record");
            });
        }
    });

    let registry = load(&location).expect("the document stays valid");
    let mut recorded: Vec<&str> = registry
        .entries()
        .iter()
        .map(|entry| entry.canonical_id().as_str())
        .collect();
    recorded.sort_unstable();
    assert_eq!(recorded, projects);
}
