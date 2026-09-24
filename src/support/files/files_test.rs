use crate::boundary::host::HostEnvironment;
use crate::config::FileDeclaration;
use crate::diagnostics::{ErrorId, Result};
use crate::hash::sha256_hex;
use crate::paths;
use std::fs;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::boundary::host::{CommandOutcome, CommandSpec};
use crate::config::{HostFileSource, SandboxHomeRelativePath};
use std::cell::RefCell;
use std::collections::HashMap;

struct FakeSbx {
    /// Sandbox内のfileと、そのdigest。
    files: HashMap<String, String>,
    /// Sandbox内でsymlinkであるpath。
    symlinks: Vec<String>,
    /// 指定した終了statusで答えるinner command。
    answering: Option<(String, i32)>,
    /// `sha256sum`が返す出力そのもの。digestを含まない答えを与えるために使う。
    reported: Option<String>,
    calls: RefCell<Vec<Vec<String>>>,
    /// stdinで受け取ったbyte列。
    inputs: RefCell<Vec<Vec<u8>>>,
}

impl FakeSbx {
    fn empty() -> FakeSbx {
        FakeSbx {
            files: HashMap::new(),
            symlinks: Vec::new(),
            answering: None,
            reported: None,
            calls: RefCell::new(Vec::new()),
            inputs: RefCell::new(Vec::new()),
        }
    }

    fn holding(destination: &str, contents: &[u8]) -> FakeSbx {
        let mut files = HashMap::new();
        files.insert(destination.to_string(), sha256_hex(contents));
        FakeSbx {
            files,
            symlinks: Vec::new(),
            answering: None,
            reported: None,
            calls: RefCell::new(Vec::new()),
            inputs: RefCell::new(Vec::new()),
        }
    }

    /// destinationは在るが、`sha256sum`が指定の出力で答えるhost。
    fn reporting(destination: &str, output: &str) -> FakeSbx {
        let mut files = HashMap::new();
        files.insert(destination.to_string(), String::new());
        FakeSbx {
            files,
            symlinks: Vec::new(),
            answering: None,
            reported: Some(output.to_string()),
            calls: RefCell::new(Vec::new()),
            inputs: RefCell::new(Vec::new()),
        }
    }

    /// 指定したpathをsymlinkとして扱う。
    fn linking(mut self, path: &str) -> FakeSbx {
        self.symlinks.push(path.to_string());
        self
    }

    /// 指定したinner commandを失敗させる。
    fn failing(mut self, command: &str) -> FakeSbx {
        self.answering = Some((command.to_string(), 1));
        self
    }

    /// 指定したinner commandを任意の終了statusで答えさせる。
    fn answering(mut self, command: &str, code: i32) -> FakeSbx {
        self.answering = Some((command.to_string(), code));
        self
    }

    fn calls(&self) -> Vec<Vec<String>> {
        self.calls.borrow().clone()
    }

    fn inputs(&self) -> Vec<Vec<u8>> {
        self.inputs.borrow().clone()
    }

    /// 宣言fileをstdinで受け取って置いたか。
    fn placed(&self) -> bool {
        !self.inputs().is_empty()
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
        if let Some(input) = spec.input() {
            self.inputs.borrow_mut().push(input.to_vec());
        }
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
        let invocation = inner.join(" ");
        if let Some((needle, answered)) = &self.answering
            && invocation.contains(needle)
        {
            code = *answered;
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
fn a_declared_file_is_sent_through_stdin_and_moved_into_place() -> Checked {
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
        calls
            .iter()
            .any(|args| args.contains(&"install".to_string())
                && args.contains(&"0700".to_string())
                && args.contains(&"/home/agent/.config/example".to_string())),
        "the parent directory is private: {calls:?}"
    );
    // 中身はhostからSandboxへ、`sbx exec`のstdinだけで運ぶ。
    let destination = "/home/agent/.config/example/settings.yaml".to_string();
    let expected: Vec<String> = [
        "exec",
        "-i",
        "--user",
        "root",
        "sbxm-example",
        "--",
        "sh",
        "-c",
        PLACE_FROM_STDIN,
        "sh",
        &destination,
        &format!("{destination}.sbxm-new"),
        &sha256_hex(b"declared = true\n"),
    ]
    .iter()
    .map(|arg| (*arg).to_string())
    .collect();
    assert!(calls.contains(&expected), "{calls:?}");
    assert_eq!(host.inputs(), vec![b"declared = true\n".to_vec()]);
    Ok(())
}

#[test]
fn the_sandbox_side_steps_keep_the_file_private_and_replace_it_by_a_rename() {
    // 受け取ったbyte列は、rootだけが読める推測できない名前の一時fileへ書く。
    assert!(PLACE_FROM_STDIN.contains("umask 077"));
    assert!(PLACE_FROM_STDIN.contains("mktemp"));
    // 中身のdigestが一致した場合だけ、agentだけが読めるfileとして置く。
    assert!(PLACE_FROM_STDIN.contains("sha256sum"));
    assert!(PLACE_FROM_STDIN.contains("install -o agent -g agent -m 0600"));
    // 読み手へ半端な内容を見せない。
    assert!(PLACE_FROM_STDIN.ends_with(r#"mv -f "$2" "$1""#));
    assert!(PLACE_FROM_STDIN.contains(&format!("exit {TRANSFER_INCOMPLETE}")));
}

#[test]
fn a_failed_placement_still_removes_what_it_left_pending() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = source_file(dir.path(), b"declared = true\n")?;
    let host = FakeSbx::empty().failing("mktemp");

    let error = place_all(
        &host,
        "sbxm-example",
        &[declaration(&source, ".config/example/settings.yaml")?],
        Conflict::Refuse,
    )
    .refused_because("the placement failed inside the sandbox")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandFailed));

    let pending = "/home/agent/.config/example/settings.yaml.sbxm-new".to_string();
    assert!(
        host.calls()
            .iter()
            .any(|args| args.contains(&"rm".to_string()) && args.contains(&pending)),
        "the pending copy is removed on the way out: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn content_that_did_not_arrive_whole_is_refused_by_its_own_reason() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = source_file(dir.path(), b"declared = true\n")?;
    // Sandboxの中で受け取ったbyte列のdigestが一致しなかった。
    let host = FakeSbx::empty().answering("mktemp", TRANSFER_INCOMPLETE);

    let error = place_all(
        &host,
        "sbxm-example",
        &[declaration(&source, ".config/example/settings.yaml")?],
        Conflict::Refuse,
    )
    .refused_because("a partial file is never put in place")?;
    let diagnostic = error.diagnostics().first().required()?;
    assert_eq!(diagnostic.id, ErrorId::DeclaredFileTransferIncomplete);
    assert_eq!(
        diagnostic
            .remediation
            .as_ref()
            .and_then(|remediation| remediation.explanation.first())
            .map(|message| message.id),
        Some("remediation-declared-file-transfer-incomplete")
    );
    Ok(())
}

#[test]
fn a_source_that_changed_after_its_placement_was_decided_is_not_sent() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = source_file(dir.path(), b"declared = true\n")?;
    let host = FakeSbx::empty();
    let planned = plan_all(
        &host,
        "sbxm-example",
        &[declaration(&source, ".config/example/settings.yaml")?],
        Conflict::Refuse,
    )
    .required()?;
    fs::write(&source, b"changed afterwards\n").required()?;

    let error = planned[0]
        .carry_out(&host, "sbxm-example")
        .refused_because("the decision was made for other content")?;
    assert_eq!(error.first_id(), Some(ErrorId::DeclaredFileUnusable));
    assert_eq!(
        refused_reason_of(&error)?,
        "cause-declared-file-changed-while-placing"
    );
    assert!(!host.placed(), "nothing is sent: {:?}", host.calls());
    Ok(())
}

/// 診断が示した、sbxm自身が観測した理由のmessage ID。
fn refused_reason_of(error: &crate::diagnostics::Error) -> Checked<&'static str> {
    error
        .diagnostics()
        .first()
        .required()?
        .facts
        .iter()
        .find_map(|fact| match fact {
            crate::design::Fact::Translated { value, .. } => Some(value.id),
            _ => None,
        })
        .required_because("the observed reason is named")
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
        !host.placed(),
        "nothing is copied when the content already matches"
    );
    Ok(())
}

#[test]
fn read_only_observation_distinguishes_missing_matching_and_conflicting_files() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let contents = b"declared = true\n";
    let source = source_file(dir.path(), contents)?;
    let declarations = [declaration(&source, ".config/example/settings.yaml")?];

    assert_eq!(
        observe(&FakeSbx::empty(), "sbxm-example", &declarations)?[0].placement,
        Placement::Placed,
        "a missing destination still needs placement"
    );
    assert_eq!(
        observe(
            &FakeSbx::holding("/home/agent/.config/example/settings.yaml", contents),
            "sbxm-example",
            &declarations,
        )?[0]
            .placement,
        Placement::Unchanged,
        "a matching destination is already complete"
    );

    let host = FakeSbx::holding("/home/agent/.config/example/settings.yaml", b"older\n");
    let error = observe(&host, "sbxm-example", &declarations)
        .refused_because("a conflicting destination is never overwritten")?;
    assert_eq!(error.first_id(), Some(ErrorId::DeclaredFileConflict));
    assert!(!host.placed(), "observation never mutates the sandbox");
    Ok(())
}

#[test]
fn an_unanswered_destination_probe_is_not_observed_as_a_missing_file() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = source_file(dir.path(), b"declared = true\n")?;
    let declarations = [declaration(&source, ".config/example/settings.yaml")?];
    let host = FakeSbx::empty().answering("test -e", 126);

    let error = observe(&host, "sbxm-example", &declarations)
        .refused_because("an unanswered existence probe is not a missing artifact")?;
    assert_eq!(error.first_id(), Some(ErrorId::SandboxCheckUnobservable));
    assert!(!host.ran("sha256sum"), "an unknown destination is not read");
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
    assert!(!host.placed());

    let host = FakeSbx::holding("/home/agent/.config/example/settings.yaml", b"older\n");
    let placed = place_all(&host, "sbxm-example", &declarations, Conflict::Overwrite)
        .required_because("an explicit re-placement replaces it")?;
    assert_eq!(placed[0].placement, Placement::Placed);
    assert!(host.placed());
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
                !host.placed() && !host.ran("install"),
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

#[test]
fn the_placement_results_keep_distinct_untranslated_spellings() {
    // 表示層はこの語をそのまま出す。訳語ではなく値であるため、ここで固定する。
    assert_eq!(Placement::Placed.as_str(), "placed");
    assert_eq!(Placement::Unchanged.as_str(), "unchanged");
    assert_eq!(Placement::Modified.as_str(), "modified");
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

#[test]
fn a_destination_probe_that_did_not_answer_never_permits_a_copy() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = source_file(dir.path(), b"declared = true\n")?;

    for code in [2, 125, 126, 127] {
        let host = FakeSbx::empty().answering("test -e", code);
        let error = place_all(
            &host,
            "sbxm-example",
            &[declaration(&source, ".config/example/settings.yaml")?],
            Conflict::Refuse,
        )
        .refused_because("an unanswered existence probe does not mean the file is absent")?;

        assert_eq!(error.first_id(), Some(ErrorId::SandboxCheckUnobservable));
        assert!(
            !host.placed() && !host.ran("install"),
            "exit {code} stops before every mutation: {:?}",
            host.calls()
        );
    }
    Ok(())
}

#[test]
fn a_symlink_probe_that_did_not_answer_never_permits_a_copy() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = source_file(dir.path(), b"declared = true\n")?;

    for code in [2, 125, 126, 127] {
        let host = FakeSbx::empty().answering("test -h", code);
        let error = place_all(
            &host,
            "sbxm-example",
            &[declaration(&source, ".config/example/settings.yaml")?],
            Conflict::Refuse,
        )
        .refused_because("an unanswered symlink probe does not establish a safe path")?;

        assert_eq!(error.first_id(), Some(ErrorId::SandboxCheckUnobservable));
        assert!(
            !host.placed() && !host.ran("install"),
            "exit {code} stops before every mutation: {:?}",
            host.calls()
        );
    }
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

fn baseline_entry(destination: &str, sha256: &str) -> crate::metadata::InitialProvisioningFile {
    crate::metadata::InitialProvisioningFile {
        source: "/home/user/original.yaml".to_string(),
        destination: destination.to_string(),
        sha256: sha256.to_string(),
    }
}

#[test]
fn a_baseline_entry_absent_from_the_sandbox_is_observed_as_missing() -> Checked {
    let host = FakeSbx::empty();
    let baseline = vec![baseline_entry(".gitconfig", &sha256_hex(b"original\n"))];
    let observed = observe_against_baseline(&host, "sbxm-example", &baseline, Divergence::Conflict)
        .required_because("an absent destination is observed, not an error")?;
    assert_eq!(observed[0].placement, Placement::Placed);
    Ok(())
}

#[test]
fn a_baseline_entry_matching_the_sandboxs_digest_is_unchanged() -> Checked {
    // baselineの照合はSandbox内のdigestとだけ行い、生きているsourceは一切読まない。
    let host = FakeSbx::holding("/home/agent/.gitconfig", b"original\n");
    let baseline = vec![baseline_entry(".gitconfig", &sha256_hex(b"original\n"))];
    let observed = observe_against_baseline(&host, "sbxm-example", &baseline, Divergence::Conflict)
        .required_because("a digest that matches the baseline is unchanged")?;
    assert_eq!(observed[0].placement, Placement::Unchanged);
    assert!(
        !host.ran("/home/user/original.yaml"),
        "the live source path is never read: {:?}",
        host.calls()
    );
    Ok(())
}

#[test]
fn a_baseline_entry_that_differs_from_the_sandbox_is_a_conflict() -> Checked {
    let host = FakeSbx::holding("/home/agent/.gitconfig", b"different in the sandbox\n");
    let baseline = vec![baseline_entry(".gitconfig", &sha256_hex(b"original\n"))];
    let error = observe_against_baseline(&host, "sbxm-example", &baseline, Divergence::Conflict)
        .refused_because("existing different content is not silently overwritten")?;
    assert_eq!(error.first_id(), Some(ErrorId::DeclaredFileConflict));
    Ok(())
}

#[test]
fn a_baseline_entry_changed_after_completion_is_reported_as_modified() -> Checked {
    // 完成後のbaselineはsbxmが最後に置いた内容である。異なる内容は利用者の編集である。
    let host = FakeSbx::holding("/home/agent/.gitconfig", b"edited in the sandbox\n");
    let baseline = vec![baseline_entry(".gitconfig", &sha256_hex(b"original\n"))];
    let observed = observe_against_baseline(&host, "sbxm-example", &baseline, Divergence::Modified)
        .required_because("an edited file is observed, not refused")?;
    assert_eq!(observed[0].placement, Placement::Modified);
    assert!(!host.placed(), "observation never mutates the sandbox");
    Ok(())
}

/// sbxmが`.config/example/settings.yaml`へ最後に置いた内容を`placed`とするbaseline。
fn placed_last(placed: &[u8]) -> Vec<crate::metadata::InitialProvisioningFile> {
    vec![baseline_entry(
        ".config/example/settings.yaml",
        &sha256_hex(placed),
    )]
}

#[test]
fn a_file_still_as_sbxm_placed_it_is_replaced_by_the_new_declaration() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = source_file(dir.path(), b"new contents\n")?;
    let declarations = [declaration(&source, ".config/example/settings.yaml")?];
    let host = FakeSbx::holding("/home/agent/.config/example/settings.yaml", b"older\n");
    let baseline = placed_last(b"older\n");

    let placed = place_all(
        &host,
        "sbxm-example",
        &declarations,
        Conflict::Protect(&baseline),
    )
    .required_because("nothing would be lost by replacing what sbxm placed")?;
    assert_eq!(placed[0].placement, Placement::Placed);
    assert!(host.placed());
    Ok(())
}

#[test]
fn a_file_changed_inside_the_sandbox_is_not_replaced_while_protected() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = source_file(dir.path(), b"new contents\n")?;
    let declarations = [declaration(&source, ".config/example/settings.yaml")?];
    let host = FakeSbx::holding(
        "/home/agent/.config/example/settings.yaml",
        b"edited in the sandbox\n",
    );
    let baseline = placed_last(b"older\n");

    let error = place_all(
        &host,
        "sbxm-example",
        &declarations,
        Conflict::Protect(&baseline),
    )
    .refused_because("replacing it would lose the edit")?;
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("one refusal")?;
    assert_eq!(diagnostic.id, ErrorId::DeclaredFileModified);
    assert_eq!(
        diagnostic
            .remediation
            .as_ref()
            .and_then(|remediation| remediation.explanation.first())
            .map(|message| message.id),
        Some("remediation-declared-file-overwrite"),
        "only the user can decide to replace it"
    );
    assert!(!host.placed(), "nothing is copied: {:?}", host.calls());
    Ok(())
}

#[test]
fn a_file_sbxm_has_no_record_of_placing_is_not_replaced_while_protected() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = source_file(dir.path(), b"new contents\n")?;
    let declarations = [declaration(&source, ".config/example/settings.yaml")?];
    let host = FakeSbx::holding("/home/agent/.config/example/settings.yaml", b"older\n");

    // 記録が無ければ、その内容をsbxmが置いたのかどうか分からない。
    let error = place_all(&host, "sbxm-example", &declarations, Conflict::Protect(&[]))
        .refused_because("an unknown file is not replaced")?;
    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("one refusal")?;
    assert_eq!(diagnostic.id, ErrorId::DeclaredFileConflict);
    assert_eq!(diagnostic.description.id, "error-declared-file-unrecorded");
    assert_eq!(
        diagnostic
            .remediation
            .as_ref()
            .and_then(|remediation| remediation.explanation.first())
            .map(|message| message.id),
        Some("remediation-declared-file-overwrite")
    );
    assert!(!host.placed());
    Ok(())
}

#[test]
fn a_baseline_spelled_with_a_leading_dot_segment_still_names_the_same_destination() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = source_file(dir.path(), b"new contents\n")?;
    let declarations = [declaration(&source, ".config/example/settings.yaml")?];
    let host = FakeSbx::holding("/home/agent/.config/example/settings.yaml", b"older\n");
    let baseline = vec![baseline_entry(
        "./.config/example/settings.yaml",
        &sha256_hex(b"older\n"),
    )];

    let placed = place_all(
        &host,
        "sbxm-example",
        &declarations,
        Conflict::Protect(&baseline),
    )
    .required_because("the record names the same file")?;
    assert_eq!(placed[0].placement, Placement::Placed);
    Ok(())
}

#[test]
fn every_file_that_cannot_be_placed_is_named_before_anything_is_placed() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let fresh = dir.path().join("fresh.yaml");
    fs::write(&fresh, b"fresh\n").required()?;
    let first = dir.path().join("first.yaml");
    fs::write(&first, b"first\n").required()?;
    let second = dir.path().join("second.yaml");
    fs::write(&second, b"second\n").required()?;
    let declarations = [
        declaration(&fresh, ".config/fresh.yaml")?,
        declaration(&first, ".config/first.yaml")?,
        declaration(&second, ".config/second.yaml")?,
    ];
    let mut host = FakeSbx::empty();
    for destination in [
        "/home/agent/.config/first.yaml",
        "/home/agent/.config/second.yaml",
    ] {
        host.files
            .insert(destination.to_string(), sha256_hex(b"something else\n"));
    }

    let error = place_all(&host, "sbxm-example", &declarations, Conflict::Refuse)
        .refused_because("two destinations hold other content")?;
    let refused: Vec<ErrorId> = error.diagnostics().iter().map(|d| d.id).collect();
    assert_eq!(
        refused,
        vec![ErrorId::DeclaredFileConflict, ErrorId::DeclaredFileConflict],
        "both refusals are shown at once"
    );
    // 置ける宣言があっても、ほかの宣言を置けないと分かった時点で1件も置かない。
    assert!(!host.placed(), "nothing is copied: {:?}", host.calls());
    Ok(())
}

/// `PLACE_FROM_STDIN`をこのhostのshellで走らせる場所。
///
/// ownerを変える`install`はrootでしか通らないため、写すだけの`install`を`PATH`の先頭へ
/// 置く。一時fileは`TMPDIR`へ作らせ、残ったかどうかを確かめる。
struct Placing {
    dir: tempfile::TempDir,
}

impl Placing {
    fn new() -> Checked<Placing> {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempfile::tempdir().required()?;
        fs::create_dir(dir.path().join("bin")).required()?;
        fs::create_dir(dir.path().join("tmp")).required()?;
        let install = dir.path().join("bin/install");
        fs::write(
            &install,
            "#!/bin/sh\nwhile [ $# -gt 2 ]; do case \"$1\" in -o|-g|-m) shift 2 ;; *) break ;; esac; done\ncp \"$1\" \"$2\"\n",
        )
        .required()?;
        fs::set_permissions(&install, fs::Permissions::from_mode(0o755)).required()?;
        Ok(Placing { dir })
    }

    fn destination(&self) -> PathBuf {
        self.dir.path().join("settings.yaml")
    }

    fn command(&self, digest: &str) -> std::process::Command {
        let destination = self.destination();
        let mut command = std::process::Command::new("sh");
        command
            .args(["-c", PLACE_FROM_STDIN, "sh"])
            .arg(&destination)
            .arg(destination.with_extension("sbxm-new"))
            .arg(digest)
            .env(
                "PATH",
                format!(
                    "{}:{}",
                    self.dir.path().join("bin").display(),
                    std::env::var("PATH").unwrap_or_default()
                ),
            )
            .env("TMPDIR", self.dir.path().join("tmp"))
            .stdin(std::process::Stdio::piped());
        command
    }

    /// 一時fileの置き場に残ったもの。
    fn staged(&self) -> Checked<usize> {
        Ok(fs::read_dir(self.dir.path().join("tmp"))
            .required()?
            .count())
    }

    fn run(&self, digest: &str, input: &[u8]) -> Checked<std::process::ExitStatus> {
        use std::io::Write;

        let mut child = self.command(digest).spawn().required()?;
        child.stdin.take().required()?.write_all(input).required()?;
        child.wait().required()
    }
}

#[test]
fn the_placement_script_places_only_bytes_that_arrived_whole() -> Checked {
    let placing = Placing::new()?;
    let body = b"declared = true\n";

    let status = placing.run(&sha256_hex(body), body)?;
    assert!(status.success(), "{status:?}");
    assert_eq!(fs::read(placing.destination()).required()?, body);
    assert_eq!(
        placing.staged()?,
        0,
        "nothing is left in the temporary place"
    );
    assert!(!placing.destination().with_extension("sbxm-new").exists());

    // 欠けて届いた内容では、置いてあるfileを置き換えない。
    let status = placing.run(&sha256_hex(body), b"declared")?;
    assert_eq!(status.code(), Some(TRANSFER_INCOMPLETE));
    assert_eq!(fs::read(placing.destination()).required()?, body);
    assert_eq!(placing.staged()?, 0);
    Ok(())
}

#[test]
fn a_placement_stopped_by_a_signal_leaves_nothing_behind() -> Checked {
    // 受け取りの途中でsignalを受けても、秘密を含みうる一時fileを残さない。
    let placing = Placing::new()?;
    let mut child = placing.command(&sha256_hex(b"x")).spawn().required()?;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while placing.staged()? == 0 && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(placing.staged()?, 1, "the script started receiving");

    let pid = rustix::process::Pid::from_child(&child);
    rustix::process::kill_process(pid, rustix::process::Signal::TERM).required()?;
    child.wait().required()?;

    assert_eq!(placing.staged()?, 0);
    assert!(!placing.destination().exists());
    Ok(())
}

#[test]
fn a_sandbox_copy_that_changes_while_it_is_read_is_refused() -> Checked {
    let dir = tempfile::tempdir().required()?;
    let source = source_file(dir.path(), b"declared = true\n")?;
    let incoming = dir.path().join("incoming");
    // 観測したdigestと、`cat`が返した内容が一致しない。
    let host = FakeSbx::holding("/home/agent/.gitconfig", b"observed\n");

    let error = receive_copy(
        &host,
        "sbxm-example",
        &declaration(&source, ".gitconfig")?,
        &incoming,
    )
    .refused_because("the copy changed while it was read")?;
    assert_eq!(error.first_id(), Some(ErrorId::DeclaredFileUnusable));
    assert!(
        fs::read_dir(&incoming).required()?.next().is_none(),
        "nothing is kept"
    );

    // Sandboxに無いfileは、受け取るものが無い。
    let absent = receive_copy(
        &FakeSbx::empty(),
        "sbxm-example",
        &declaration(&source, ".gitconfig")?,
        &incoming,
    )
    .required()?;
    assert!(absent.is_none());
    Ok(())
}
