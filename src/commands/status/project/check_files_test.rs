use std::fs;

use crate::config::{FileDeclaration, GlobalConfig, HostFileSource, SandboxHomeRelativePath};
use crate::diagnostics::ErrorId;
use crate::hash::sha256_hex;

use crate::testing::add_request::{project_of, request};
use crate::testing::outcome::{Checked, Required};
use crate::testing::provisioning::{Bench, World};

use super::super::{FileRow, FileState, ProjectStatus, diagnose};

const DESTINATION: &str = ".config/example/settings.yaml";
const IN_SANDBOX: &str = "/home/agent/.config/example/settings.yaml";

/// 宣言file 1件を置いて初回構築まで終えた案件。
fn built() -> Checked<(Bench, World)> {
    let bench = Bench::new()?;
    let world = World::new();
    bench
        .build(&world, &request("Example-Org/Example-Repo", None, None)?)
        .required_because("the first build completes")?;
    Ok((bench, world))
}

fn diagnosed(bench: &Bench, config: &GlobalConfig, world: &World) -> Checked<ProjectStatus> {
    diagnose(
        &bench.location,
        config,
        &project_of(&request("Example-Org/Example-Repo", None, None)?)?,
        world,
        bench.workspace_root.path(),
    )
    .required_because("diagnose the built project")
}

fn row(host: FileState, sandbox: FileState) -> FileRow {
    FileRow {
        destination: DESTINATION.to_string(),
        host,
        sandbox,
    }
}

#[test]
fn a_file_nobody_touched_is_unchanged_on_both_sides() -> Checked {
    let (bench, world) = built()?;
    let status = diagnosed(&bench, &bench.config, &world)?;
    assert_eq!(
        status.files,
        vec![row(FileState::Unchanged, FileState::Unchanged)]
    );
    // 宣言fileについて示すことは無い。worktreeの観測はこのtestの対象ではない。
    assert!(
        !status.diagnostics.iter().any(|diagnostic| matches!(
            diagnostic.id,
            ErrorId::DeclaredFileUnusable | ErrorId::SandboxCheckUnobservable
        )),
        "{:?}",
        status.diagnostics
    );
    Ok(())
}

#[test]
fn each_side_is_compared_with_what_sbxm_placed_last() -> Checked {
    // hostで宣言fileを書き換えた。次の`apply --files`がそれを置く。
    let (bench, world) = built()?;
    fs::write(
        bench.config.files[0].source.as_path(),
        b"edited on the host\n",
    )
    .required()?;
    assert_eq!(
        diagnosed(&bench, &bench.config, &world)?.files,
        vec![row(FileState::Updated, FileState::Unchanged)]
    );

    // Sandboxの中で書き換えた。`apply --files`はそれを置き換えない。
    let (bench, world) = built()?;
    world.digests.borrow_mut().insert(
        IN_SANDBOX.to_string(),
        sha256_hex(b"edited inside the sandbox\n"),
    );
    assert_eq!(
        diagnosed(&bench, &bench.config, &world)?.files,
        vec![row(FileState::Unchanged, FileState::Modified)]
    );

    // Sandboxから消えた。
    let (bench, world) = built()?;
    world.present.borrow_mut().remove(IN_SANDBOX);
    world.digests.borrow_mut().remove(IN_SANDBOX);
    assert_eq!(
        diagnosed(&bench, &bench.config, &world)?.files,
        vec![row(FileState::Unchanged, FileState::Missing)]
    );
    Ok(())
}

#[test]
fn a_declaration_added_after_the_build_has_not_been_placed_yet() -> Checked {
    let (bench, world) = built()?;
    let notes = bench.config.files[0]
        .source
        .as_path()
        .with_file_name("notes.md");
    fs::write(&notes, b"# notes\n").required()?;
    let mut config = bench.config.clone();
    config.files.push(FileDeclaration {
        source: HostFileSource::new(&crate::paths::display(&notes)).required()?,
        destination: SandboxHomeRelativePath::new("notes.md").required()?,
    });

    let status = diagnosed(&bench, &config, &world)?;
    assert_eq!(
        status.files[1],
        FileRow {
            destination: "notes.md".to_string(),
            host: FileState::Unplaced,
            sandbox: FileState::Missing,
        }
    );

    // 記録が無くても、Sandboxの中身がhostのfileと同じなら置き直す必要は無い。
    world
        .digests
        .borrow_mut()
        .insert("/home/agent/notes.md".to_string(), sha256_hex(b"# notes\n"));
    world
        .present
        .borrow_mut()
        .insert("/home/agent/notes.md".to_string());
    assert_eq!(
        diagnosed(&bench, &config, &world)?.files[1].sandbox,
        FileState::Unchanged
    );

    // 異なる中身は、sbxmが置いたものかどうか分からない。
    world
        .digests
        .borrow_mut()
        .insert("/home/agent/notes.md".to_string(), sha256_hex(b"other\n"));
    assert_eq!(
        diagnosed(&bench, &config, &world)?.files[1].sandbox,
        FileState::Unrecorded
    );
    Ok(())
}

#[test]
fn a_host_file_that_cannot_be_placed_is_unreadable_and_says_why() -> Checked {
    let (bench, world) = built()?;
    fs::remove_file(bench.config.files[0].source.as_path()).required()?;

    let status = diagnosed(&bench, &bench.config, &world)?;
    assert_eq!(status.files[0].host, FileState::Unreadable);
    assert!(
        status
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.id == ErrorId::DeclaredFileUnusable),
        "{:?}",
        status.diagnostics
    );
    assert!(!status.is_healthy());
    Ok(())
}

#[test]
fn a_sandbox_that_is_not_running_is_not_looked_into() -> Checked {
    let (bench, world) = built()?;
    world.stopped();
    let mark = world.mark();

    let status = diagnosed(&bench, &bench.config, &world)?;
    assert_eq!(status.files[0].sandbox, FileState::NotObservedStopped);
    assert!(
        !world
            .since(mark)
            .iter()
            .any(|call| call.contains("sha256sum")),
        "a stopped sandbox is not started to read it"
    );

    // Sandboxの無い案件には、見に行く配置先が無い。
    let bench = Bench::new()?;
    let world = World::new();
    bench
        .register(&world, &request("Example-Org/Example-Repo", None, None)?)
        .required()?;
    assert_eq!(
        diagnosed(&bench, &bench.config, &world)?.files,
        vec![row(FileState::Unplaced, FileState::NotApplicable)]
    );
    Ok(())
}

#[test]
fn a_sandbox_that_does_not_answer_is_not_read_as_missing() -> Checked {
    let (bench, world) = built()?;
    // `sha256sum`が成功しながらdigestを返さない。無いのか違うのかは分からない。
    world.succeeding_silently("sha256sum");

    let status = diagnosed(&bench, &bench.config, &world)?;
    assert_eq!(status.files[0].sandbox, FileState::NotObserved);
    assert!(!status.is_healthy(), "the unanswered check is reported");
    Ok(())
}

#[test]
fn a_sandbox_whose_state_was_not_observed_is_not_looked_into() -> Checked {
    let (bench, world) = built()?;
    let project = project_of(&request("Example-Org/Example-Repo", None, None)?)?;
    let metadata = crate::support::select::find(&bench.location, &project)
        .required()?
        .reload()
        .required()?;
    let mut status = ProjectStatus {
        project: metadata.display_id(),
        items: Vec::new(),
        worktrees: Vec::new(),
        files: Vec::new(),
        disk: crate::support::disk::DiskObservation::NotObservedMismatch,
        diagnostics: Vec::new(),
        next: None,
    };
    let mark = world.mark();

    super::check_files(
        &world,
        &metadata.sandbox_name(),
        &metadata,
        &bench.config,
        None,
        &mut status,
    );

    assert_eq!(
        status.files,
        vec![row(FileState::Unchanged, FileState::NotObserved)]
    );
    assert!(world.since(mark).is_empty(), "{:?}", world.since(mark));
    Ok(())
}
