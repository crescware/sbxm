use crate::command::HostEnvironment;
use crate::config::FileDeclaration;
use crate::diagnostics::{ErrorId, Result};
use crate::hash::sha256_hex;
use crate::paths;
use std::fs;
use std::path::{Path, PathBuf};

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::command::{CommandOutcome, CommandSpec};
use crate::config::{HostFileSource, SandboxHomeRelativePath};
use std::cell::RefCell;
use std::collections::HashMap;

struct FakeSbx {
    /// Sandbox内のfileと、そのdigest。
    files: HashMap<String, String>,
    /// Sandbox内でsymlinkであるpath。
    symlinks: Vec<String>,
    /// 非zeroで答えるinner command。
    failing: Option<String>,
    calls: RefCell<Vec<Vec<String>>>,
}

impl FakeSbx {
    fn empty() -> FakeSbx {
        FakeSbx {
            files: HashMap::new(),
            symlinks: Vec::new(),
            failing: None,
            calls: RefCell::new(Vec::new()),
        }
    }

    fn holding(destination: &str, contents: &[u8]) -> FakeSbx {
        let mut files = HashMap::new();
        files.insert(destination.to_string(), sha256_hex(contents));
        FakeSbx {
            files,
            symlinks: Vec::new(),
            failing: None,
            calls: RefCell::new(Vec::new()),
        }
    }

    /// 指定したpathをsymlinkとして扱う。
    fn linking(mut self, path: &str) -> FakeSbx {
        self.symlinks.push(path.to_string());
        self
    }

    /// 指定したinner commandを失敗させる。
    fn failing(mut self, command: &str) -> FakeSbx {
        self.failing = Some(command.to_string());
        self
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.borrow().clone()
    }

    fn ran(&self, needle: &str) -> bool {
        self.calls()
            .iter()
            .any(|args| args.iter().any(|arg| arg == needle))
    }
}

impl HostEnvironment for FakeSbx {
    fn command_exists(&self, _program: &str) -> bool {
        true
    }

    fn run(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.calls.borrow_mut().push(spec.args.clone());
        let mut code = 0;
        let mut stdout = String::new();

        let inner = crate::testing::command::inner_args(spec);
        match inner.first().copied() {
            Some("test") => {
                let target = inner.last().copied().unwrap_or_default();
                let present = match inner.get(1).copied() {
                    Some("-h") => self.symlinks.iter().any(|known| known == target),
                    _ => self.files.contains_key(target),
                };
                code = i32::from(!present);
            }
            Some("sha256sum") => {
                let target = inner.last().copied().unwrap_or_default();
                match self.files.get(target) {
                    Some(digest) => stdout = format!("{digest}  {target}\n"),
                    None => code = 1,
                }
            }
            _ => {}
        }
        if inner.first().is_some_and(|arg| {
            self.failing
                .as_deref()
                .is_some_and(|failing| failing == *arg)
        }) {
            code = 1;
        }

        Ok(crate::testing::command::outcome(spec, code, &stdout))
    }
}

fn declaration(source: &Path, destination: &str) -> Checked<FileDeclaration> {
    Ok(FileDeclaration {
        source: HostFileSource::new(&paths::display(source)).required_because("valid source")?,
        destination: SandboxHomeRelativePath::new(destination)
            .required_because("valid destination")?,
    })
}

fn source_file(dir: &Path, contents: &[u8]) -> Checked<PathBuf> {
    let path = dir.join("declared.yaml");
    fs::write(&path, contents).required_because("write the source")?;
    Ok(path)
}

#[test]
fn a_declared_file_is_staged_installed_and_moved_into_place() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = source_file(dir.path(), b"declared = true\n")?;
    let host = FakeSbx::empty();

    let placed = place_all(
        &host,
        "sbxm-example",
        &[declaration(&source, ".config/example/settings.yaml")?],
        Conflict::Refuse,
    )
    .required_because("place")?;

    assert_eq!(
        placed,
        vec![PlacedFile {
            source: source.clone(),
            destination: ".config/example/settings.yaml".to_string(),
            placement: Placement::Placed,
        }]
    );

    let calls = host.calls();
    assert!(
        calls.iter().any(|args| args
            == &vec![
                "cp".to_string(),
                "--follow-link".to_string(),
                paths::display(&source),
                "sbxm-example:/tmp/sbxm-file-0".to_string()
            ]),
        "{calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|args| args.contains(&"install".to_string())
                && args.contains(&"0700".to_string())
                && args.contains(&"/home/agent/.config/example".to_string())),
        "the parent directory is private: {calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|args| args.contains(&"install".to_string())
                && args.contains(&"0600".to_string())
                && args.contains(&"agent".to_string())),
        "the file belongs to the agent and is private: {calls:?}"
    );
    assert!(
        calls.iter().any(|args| args.contains(&"mv".to_string())
            && args.contains(&"/home/agent/.config/example/settings.yaml".to_string())),
        "the destination is replaced by a rename: {calls:?}"
    );
    assert!(
        host.ran("/tmp/sbxm-file-0"),
        "the staged copy is removed afterwards: {calls:?}"
    );
    Ok(())
}

#[test]
fn a_failed_placement_still_removes_what_it_staged() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = source_file(dir.path(), b"declared = true\n")?;
    let host = FakeSbx::empty().failing("mv");

    let error = place_all(
        &host,
        "sbxm-example",
        &[declaration(&source, ".config/example/settings.yaml")?],
        Conflict::Refuse,
    )
    .refused_because("the rename is the last step and it failed")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));

    let pending = "/home/agent/.config/example/settings.yaml.sbxm-new".to_string();
    let staged = "/tmp/sbxm-file-0".to_string();
    assert!(
        host.calls()
            .iter()
            .any(|args| args.contains(&"rm".to_string())
                && args.contains(&staged)
                && args.contains(&pending)),
        "both temporary files are removed on the way out: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_destination_that_already_holds_the_same_content_is_left_alone() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let contents = b"declared = true\n";
    let source = source_file(dir.path(), contents)?;
    let host = FakeSbx::holding("/home/agent/.config/example/settings.yaml", contents);

    let placed = place_all(
        &host,
        "sbxm-example",
        &[declaration(&source, ".config/example/settings.yaml")?],
        Conflict::Refuse,
    )
    .required_because("place")?;

    assert_eq!(placed[0].placement, Placement::Unchanged);
    assert!(
        !host.ran("cp"),
        "nothing is copied when the content already matches"
    );
    Ok(())
}

#[test]
fn add_refuses_to_overwrite_a_different_file_while_sync_files_replaces_it() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = source_file(dir.path(), b"new contents\n")?;
    let declarations = [declaration(&source, ".config/example/settings.yaml")?];

    let host = FakeSbx::holding("/home/agent/.config/example/settings.yaml", b"older\n");
    let error = place_all(&host, "sbxm-example", &declarations, Conflict::Refuse)
        .refused_because("a build never overwrites what is already there")?;
    assert_eq!(error.first_id(), Some(ErrorId::DeclaredFileConflict));
    assert!(!host.ran("cp"));

    let host = FakeSbx::holding("/home/agent/.config/example/settings.yaml", b"older\n");
    let placed = place_all(&host, "sbxm-example", &declarations, Conflict::Overwrite)
        .required_because("an explicit re-placement replaces it")?;
    assert_eq!(placed[0].placement, Placement::Placed);
    assert!(host.ran("mv"));
    Ok(())
}

#[test]
fn a_source_that_cannot_be_placed_safely_stops_before_anything_is_copied() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let real = source_file(dir.path(), b"declared\n")?;

    let link = dir.path().join("link.yaml");
    std::os::unix::fs::symlink(&real, &link).required()?;
    let directory = dir.path().join("a-directory");
    fs::create_dir(&directory).required()?;
    let large = dir.path().join("large.bin");
    let oversized = usize::try_from(MAX_SOURCE_BYTES + 1).required()?;
    fs::write(&large, vec![0_u8; oversized]).required()?;
    let absent = dir.path().join("absent.yaml");

    for source in [link, directory, large, absent] {
        let host = FakeSbx::empty();
        let error = place_all(
            &host,
            "sbxm-example",
            &[declaration(&source, ".config/example/settings.yaml")?],
            Conflict::Refuse,
        )
        .refused_because("{source:?} must be refused")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::DeclaredFileUnusable),
            "source {source:?} produced the wrong error"
        );
        assert!(host.calls().is_empty(), "nothing is asked of the sandbox");
    }
    Ok(())
}

#[test]
fn a_destination_reached_through_a_symbolic_link_is_refused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = source_file(dir.path(), b"declared = true\n")?;
    let declarations = [declaration(&source, ".config/example/settings.yaml")?];

    // 途中のdirectoryも、destination自身も、homeの外を指し得る。
    for link in [
        "/home/agent/.config",
        "/home/agent/.config/example",
        "/home/agent/.config/example/settings.yaml",
    ] {
        for conflict in [Conflict::Refuse, Conflict::Overwrite] {
            let host = FakeSbx::empty().linking(link);
            let error = place_all(&host, "sbxm-example", &declarations, conflict)
                .refused_because("a path that leaves the agent home is not written to")?;
            assert_eq!(
                error.first_id(),
                Some(ErrorId::DeclaredFileUnusable),
                "{link} produced the wrong error"
            );
            assert!(
                !host.ran("cp") && !host.ran("install") && !host.ran("mv"),
                "nothing is copied or installed through {link}: {:?}",
                host.calls()
            );
            assert!(
                !host.ran("sha256sum"),
                "the destination is not even read through {link}: {:?}",
                host.calls()
            );
        }
    }
    Ok(())
}

#[test]
fn every_step_of_the_destination_is_checked_for_a_symbolic_link() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = source_file(dir.path(), b"declared = true\n")?;
    let host = FakeSbx::empty();

    place_all(
        &host,
        "sbxm-example",
        &[declaration(&source, ".config/example/settings.yaml")?],
        Conflict::Refuse,
    )
    .required_because("place")?;

    for step in [
        "/home/agent/.config",
        "/home/agent/.config/example",
        "/home/agent/.config/example/settings.yaml",
    ] {
        assert!(
            host.calls()
                .iter()
                .any(|args| args.contains(&"-h".to_string()) && args.contains(&step.to_string())),
            "{step} is checked: {:?}",
            host.calls()
        );
    }
    Ok(())
}

#[test]
fn the_content_of_a_declared_file_never_reaches_a_diagnostic() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let secret = "a-value-that-must-not-be-shown";
    let source = source_file(dir.path(), secret.as_bytes())?;
    let host = FakeSbx::holding("/home/agent/.config/example/settings.yaml", b"older\n");

    let error = place_all(
        &host,
        "sbxm-example",
        &[declaration(&source, ".config/example/settings.yaml")?],
        Conflict::Refuse,
    )
    .refused_because("the conflict is reported")?;

    let rendered = format!("{error:?}");
    assert!(
        !rendered.contains(secret),
        "the diagnostic must name the paths only: {rendered}"
    );
    for args in host.calls() {
        assert!(
            !args.iter().any(|arg| arg.contains(secret)),
            "the content never reaches an argument: {args:?}"
        );
    }
    Ok(())
}
