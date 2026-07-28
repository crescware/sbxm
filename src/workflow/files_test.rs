use super::*;
use crate::command::{CommandOutcome, CommandSpec};
use crate::config::{HostFileSource, SandboxHomeRelativePath};
use std::cell::RefCell;
use std::collections::HashMap;
use std::os::unix::process::ExitStatusExt;

struct FakeSbx {
    /// Sandbox内のfileと、そのdigest。
    files: HashMap<String, String>,
    /// Sandbox内でsymlinkであるpath。
    symlinks: Vec<String>,
    calls: RefCell<Vec<Vec<String>>>,
}

impl FakeSbx {
    fn empty() -> FakeSbx {
        FakeSbx {
            files: HashMap::new(),
            symlinks: Vec::new(),
            calls: RefCell::new(Vec::new()),
        }
    }

    fn holding(destination: &str, contents: &[u8]) -> FakeSbx {
        let mut files = HashMap::new();
        files.insert(destination.to_string(), sha256_hex(contents));
        FakeSbx {
            files,
            symlinks: Vec::new(),
            calls: RefCell::new(Vec::new()),
        }
    }

    /// 指定したpathをsymlinkとして扱う。
    fn linking(mut self, path: &str) -> FakeSbx {
        self.symlinks.push(path.to_string());
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

        if let Some(position) = spec.args.iter().position(|arg| arg == "--") {
            let inner = &spec.args[position + 1..];
            match inner.first().map(String::as_str) {
                Some("test") => {
                    let target = inner.last().cloned().unwrap_or_default();
                    let present = match inner.get(1).map(String::as_str) {
                        Some("-h") => self.symlinks.contains(&target),
                        _ => self.files.contains_key(&target),
                    };
                    code = i32::from(!present);
                }
                Some("sha256sum") => {
                    let target = inner.last().cloned().unwrap_or_default();
                    match self.files.get(&target) {
                        Some(digest) => stdout = format!("{digest}  {target}\n"),
                        None => code = 1,
                    }
                }
                _ => {}
            }
        }

        Ok(CommandOutcome {
            program: spec.program.clone(),
            args: spec.args.clone(),
            working_dir: spec.working_dir.clone(),
            status: std::process::ExitStatus::from_raw(code << 8),
            stdout: stdout.into_bytes(),
            stderr: Vec::new(),
            stderr_lossy: false,
        })
    }
}

fn declaration(source: &Path, destination: &str) -> FileDeclaration {
    FileDeclaration {
        source: HostFileSource::new(&paths::display(source)).expect("valid source"),
        destination: SandboxHomeRelativePath::new(destination).expect("valid destination"),
    }
}

fn source_file(dir: &Path, contents: &[u8]) -> PathBuf {
    let path = dir.join("declared.toml");
    fs::write(&path, contents).expect("write the source");
    path
}

#[test]
fn a_declared_file_is_staged_installed_and_moved_into_place() {
    let dir = tempfile::tempdir().unwrap();
    let source = source_file(dir.path(), b"declared = true\n");
    let host = FakeSbx::empty();

    let placed = place_all(
        &host,
        "sbxm-example",
        &[declaration(&source, ".config/example/config.toml")],
        Conflict::Refuse,
    )
    .expect("place");

    assert_eq!(
        placed,
        vec![PlacedFile {
            source: source.clone(),
            destination: ".config/example/config.toml".to_string(),
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
            && args.contains(&"/home/agent/.config/example/config.toml".to_string())),
        "the destination is replaced by a rename: {calls:?}"
    );
    assert!(
        host.ran("/tmp/sbxm-file-0"),
        "the staged copy is removed afterwards: {calls:?}"
    );
}

#[test]
fn a_destination_that_already_holds_the_same_content_is_left_alone() {
    let dir = tempfile::tempdir().unwrap();
    let contents = b"declared = true\n";
    let source = source_file(dir.path(), contents);
    let host = FakeSbx::holding("/home/agent/.config/example/config.toml", contents);

    let placed = place_all(
        &host,
        "sbxm-example",
        &[declaration(&source, ".config/example/config.toml")],
        Conflict::Refuse,
    )
    .expect("place");

    assert_eq!(placed[0].placement, Placement::Unchanged);
    assert!(
        !host.ran("cp"),
        "nothing is copied when the content already matches"
    );
}

#[test]
fn add_refuses_to_overwrite_a_different_file_while_sync_files_replaces_it() {
    let dir = tempfile::tempdir().unwrap();
    let source = source_file(dir.path(), b"new contents\n");
    let declarations = [declaration(&source, ".config/example/config.toml")];

    let host = FakeSbx::holding("/home/agent/.config/example/config.toml", b"older\n");
    let error = place_all(&host, "sbxm-example", &declarations, Conflict::Refuse)
        .expect_err("a build never overwrites what is already there");
    assert_eq!(error.first_id(), Some(ErrorId::DeclaredFileConflict));
    assert!(!host.ran("cp"));

    let host = FakeSbx::holding("/home/agent/.config/example/config.toml", b"older\n");
    let placed = place_all(&host, "sbxm-example", &declarations, Conflict::Overwrite)
        .expect("an explicit re-placement replaces it");
    assert_eq!(placed[0].placement, Placement::Placed);
    assert!(host.ran("mv"));
}

#[test]
fn a_source_that_cannot_be_placed_safely_stops_before_anything_is_copied() {
    let dir = tempfile::tempdir().unwrap();
    let real = source_file(dir.path(), b"declared\n");

    let link = dir.path().join("link.toml");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let directory = dir.path().join("a-directory");
    fs::create_dir(&directory).unwrap();
    let large = dir.path().join("large.bin");
    fs::write(&large, vec![0_u8; (MAX_SOURCE_BYTES + 1) as usize]).unwrap();
    let absent = dir.path().join("absent.toml");

    for source in [link, directory, large, absent] {
        let host = FakeSbx::empty();
        let error = place_all(
            &host,
            "sbxm-example",
            &[declaration(&source, ".config/example/config.toml")],
            Conflict::Refuse,
        )
        .expect_err("{source:?} must be refused");
        assert_eq!(
            error.first_id(),
            Some(ErrorId::DeclaredFileUnusable),
            "source {source:?} produced the wrong error"
        );
        assert!(host.calls().is_empty(), "nothing is asked of the sandbox");
    }
}

#[test]
fn a_destination_reached_through_a_symbolic_link_is_refused() {
    let dir = tempfile::tempdir().unwrap();
    let source = source_file(dir.path(), b"declared = true\n");
    let declarations = [declaration(&source, ".config/example/config.toml")];

    // 途中のdirectoryも、destination自身も、homeの外を指し得る。
    for link in [
        "/home/agent/.config",
        "/home/agent/.config/example",
        "/home/agent/.config/example/config.toml",
    ] {
        for conflict in [Conflict::Refuse, Conflict::Overwrite] {
            let host = FakeSbx::empty().linking(link);
            let error = place_all(&host, "sbxm-example", &declarations, conflict)
                .expect_err("a path that leaves the agent home is not written to");
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
}

#[test]
fn every_step_of_the_destination_is_checked_for_a_symbolic_link() {
    let dir = tempfile::tempdir().unwrap();
    let source = source_file(dir.path(), b"declared = true\n");
    let host = FakeSbx::empty();

    place_all(
        &host,
        "sbxm-example",
        &[declaration(&source, ".config/example/config.toml")],
        Conflict::Refuse,
    )
    .expect("place");

    for step in [
        "/home/agent/.config",
        "/home/agent/.config/example",
        "/home/agent/.config/example/config.toml",
    ] {
        assert!(
            host.calls()
                .iter()
                .any(|args| args.contains(&"-h".to_string()) && args.contains(&step.to_string())),
            "{step} is checked: {:?}",
            host.calls()
        );
    }
}

#[test]
fn the_content_of_a_declared_file_never_reaches_a_diagnostic() {
    let dir = tempfile::tempdir().unwrap();
    let secret = "a-value-that-must-not-be-shown";
    let source = source_file(dir.path(), secret.as_bytes());
    let host = FakeSbx::holding("/home/agent/.config/example/config.toml", b"older\n");

    let error = place_all(
        &host,
        "sbxm-example",
        &[declaration(&source, ".config/example/config.toml")],
        Conflict::Refuse,
    )
    .expect_err("the conflict is reported");

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
}
