use crate::command::HostEnvironment;
use crate::config::FileDeclaration;
use crate::diagnostics::{ErrorId, Result};
use crate::hash::sha256_hex;
use crate::paths;
use std::fs;
use std::os::unix::ffi::OsStringExt;
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
    /// `sha256sum`が返す出力そのもの。digestを含まない答えを与えるために使う。
    reported: Option<String>,
    calls: RefCell<Vec<Vec<String>>>,
}

impl FakeSbx {
    fn empty() -> FakeSbx {
        FakeSbx {
            files: HashMap::new(),
            symlinks: Vec::new(),
            failing: None,
            reported: None,
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
            reported: None,
            calls: RefCell::new(Vec::new()),
        }
    }

    /// destinationは在るが、`sha256sum`が指定の出力で答えるhost。
    fn reporting(destination: &str, output: &str) -> FakeSbx {
        let mut files = HashMap::new();
        files.insert(destination.to_string(), String::new());
        FakeSbx {
            files,
            symlinks: Vec::new(),
            failing: None,
            reported: Some(output.to_string()),
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
                match (self.reported.as_deref(), self.files.get(target)) {
                    (Some(output), _) => stdout = output.to_string(),
                    (None, Some(digest)) => stdout = format!("{digest}  {target}\n"),
                    (None, None) => code = 1,
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
fn preflight_accepts_a_missing_or_matching_declared_file() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = source_file(dir.path(), b"declared = true\n")?;
    let declaration = declaration(&source, ".config/example/settings.yaml")?;

    preflight(
        &FakeSbx::empty(),
        "sbxm-example",
        std::slice::from_ref(&declaration),
    )
    .required_because("a missing destination is safe to place")?;

    let host = FakeSbx::holding(
        "/home/agent/.config/example/settings.yaml",
        b"declared = true\n",
    );
    preflight(&host, "sbxm-example", &[declaration])
        .required_because("a matching destination is safe to reuse")?;
    Ok(())
}

#[test]
fn preflight_refuses_a_declared_file_with_different_contents() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = source_file(dir.path(), b"new contents\n")?;
    let declaration = declaration(&source, ".config/example/settings.yaml")?;
    let host = FakeSbx::holding(
        "/home/agent/.config/example/settings.yaml",
        b"older contents\n",
    );

    let error = preflight(&host, "sbxm-example", &[declaration])
        .refused_because("repair does not overwrite a changed declared file")?;
    assert_eq!(error.first_id(), Some(ErrorId::DeclaredFileConflict));
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

#[test]
fn the_two_placement_results_keep_two_distinct_untranslated_spellings() {
    // 表示層はこの語をそのまま出す。訳語ではなく値であるため、ここで固定する。
    assert_eq!(Placement::Placed.as_str(), "placed");
    assert_eq!(Placement::Unchanged.as_str(), "unchanged");
    assert_ne!(
        Placement::Placed.as_str(),
        Placement::Unchanged.as_str(),
        "何もしなかった配置を、置いた配置と読み違えられてはならない"
    );
}

#[test]
fn an_answer_from_sha256sum_without_a_digest_is_refused_rather_than_read_as_one() -> Checked {
    let destination = "/home/agent/.config/example/settings.yaml";
    // 空でも、短くても、digestを含まない行でも、それはdigestではない。
    for reported in [
        "",
        "d41d8cd98f00b204  /home/agent/.config/example/settings.yaml\n",
        "sha256sum: standard input: Input/output error\n",
    ] {
        let host = FakeSbx::reporting(destination, reported);
        let error = digest_in_sandbox(&host, "sbxm-example", destination)
            .refused_because("an answer that carries no digest is not a digest")?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));

        let facts = &error.diagnostics()[0].facts;
        assert!(
            facts.iter().any(
                |fact| matches!(fact, crate::design::Fact::OneLine { label, value }
                if label.id == "diagnostic-command-label" && value.as_str() == "sha256sum")
            ),
            "the tool whose answer could not be read is named: {facts:?}"
        );
        assert!(
            facts.iter().any(
                |fact| matches!(fact, crate::design::Fact::OneLine { label, value }
                if label.id == "diagnostic-cause-label" && value.as_str().contains(destination))
            ),
            "the destination that was asked about is named: {facts:?}"
        );
    }
    Ok(())
}

#[test]
fn a_destination_that_is_not_in_the_sandbox_has_no_digest_rather_than_an_error() -> Checked {
    let destination = "/home/agent/.config/example/settings.yaml";
    let contents = b"declared = true\n";

    let host = FakeSbx::holding(destination, contents);
    assert_eq!(
        digest_in_sandbox(&host, "sbxm-example", destination)
            .required_because("the digest of a file that is there")?,
        Some(sha256_hex(contents))
    );

    // 無いfileは「無い」と答える。読めないことと無いことを同じ答えにしない。
    let host = FakeSbx::empty();
    assert_eq!(
        digest_in_sandbox(&host, "sbxm-example", destination)
            .required_because("a destination that is not there")?,
        None
    );
    assert!(
        !host.ran("sha256sum"),
        "a destination that is not there is never read: {:?}",
        host.calls()
    );
    Ok(())
}

/// 配置先として拒否された理由のmessage ID。
fn refused_reason(destination: &str) -> Checked<String> {
    let error =
        destination_path(Path::new(destination)).refused_because("the destination is refused")?;
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?;
    assert_eq!(diagnostic.id, ErrorId::DeclaredFileUnusable);
    diagnostic
        .facts
        .iter()
        .find_map(|fact| match fact {
            crate::design::Fact::Translated { value, .. } => Some(value.id.to_string()),
            _ => None,
        })
        .required_because("the observed reason is named")
}

#[test]
fn a_destination_that_stays_under_the_agent_home_is_joined_with_slashes() -> Checked {
    assert_eq!(
        destination_path(Path::new(".config/example/settings.yaml")).required()?,
        ".config/example/settings.yaml"
    );
    // `./`は位置を変えないため、部品として残さない。
    assert_eq!(
        destination_path(Path::new("./.gitconfig")).required()?,
        ".gitconfig"
    );
    Ok(())
}

#[test]
fn a_destination_that_leaves_the_agent_home_is_refused_by_its_own_reason() -> Checked {
    assert_eq!(
        refused_reason("/etc/passwd")?,
        "cause-unexpectedly-absolute"
    );
    assert_eq!(refused_reason("../outside")?, "cause-leaves-agent-home");
    assert_eq!(refused_reason(".")?, "cause-value-empty");
    Ok(())
}

#[test]
fn a_destination_that_is_not_utf8_is_refused_rather_than_placed() -> Checked {
    // Sandbox内のpathは文字列として渡すため、UTF-8で表せない値は配置できない。
    let raw = PathBuf::from(std::ffi::OsString::from_vec(b"\xff\xfe".to_vec()));
    let error = destination_path(&raw).refused_because("a name that is not UTF-8")?;
    let reason = error.diagnostics()[0]
        .facts
        .iter()
        .find_map(|fact| match fact {
            crate::design::Fact::Translated { value, .. } => Some(value.id),
            _ => None,
        })
        .required_because("the observed reason is named")?;
    assert_eq!(reason, "cause-not-valid-utf8");
    Ok(())
}
