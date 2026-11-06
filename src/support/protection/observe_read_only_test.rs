use std::cell::RefCell;
use std::fmt::Write;
use std::time::Duration;

use crate::boundary::host::{CommandOutcome, CommandSpec, ReadOnly};
use crate::diagnostics::Result;
use crate::testing::command::outcome;
use crate::testing::host::{AnsweredHost, FakeSbx};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::scripted_clock::ScriptedClock;
use crate::testing::value::COMMIT;
use crate::time::{Clock, Moment};

use super::*;

struct ManyRefs {
    refs: usize,
    duplicate: bool,
    clock: ScriptedClock,
    calls: RefCell<Vec<CommandSpec>>,
}

impl AnsweredHost for ManyRefs {
    fn has_command(&self, _program: &str) -> bool {
        true
    }
    fn answer(&self, spec: &CommandSpec) -> Result<CommandOutcome> {
        self.clock.advance(Duration::from_millis(90));
        self.calls.borrow_mut().push(spec.clone());
        let args = spec.args.join(" ");
        let output = if args.ends_with("remote.origin.url") {
            "https://github.com/owner/repo.git\n".to_string()
        } else if args.contains("--format=%(refname)%09%(objectname)") {
            let mut output = String::new();
            for index in 0..self.refs {
                let _ = writeln!(
                    output,
                    "refs/remotes/origin/branch-{index}\t{:040x}",
                    if self.duplicate { 1 } else { index + 1 }
                );
            }
            output
        } else if args.contains("--batch-check=") {
            String::from_utf8_lossy(spec.input().unwrap_or_default()).into_owned()
        } else if args.contains("--contains=") {
            "refs/remotes/origin/branch-0\n".to_string()
        } else {
            String::new()
        };
        Ok(outcome(spec, 0, &output))
    }
}

#[test]
fn hundreds_of_refs_use_one_object_query_and_a_constant_number_of_sandbox_calls() -> Checked {
    for refs in [1, 400, 2000] {
        let host = ManyRefs {
            refs,
            duplicate: false,
            clock: ScriptedClock::default(),
            calls: RefCell::new(Vec::new()),
        };
        let id = crate::project::ProjectId::parse("owner/repo")
            .required()?
            .canonical();
        let sandbox = SandboxName::derive(&id);
        let layout = SandboxLayout::new(&id);
        let candidates: Vec<_> = (0..5)
            .map(|index| {
                CommitCandidate::new(
                    format!("refs/heads/work-{index}"),
                    format!("{:040x}", 10_000 + index),
                    None,
                )
            })
            .collect();
        let observed = observe_read_only(
            &host,
            &sandbox,
            &layout,
            &candidates,
            Path::new("/host/repo"),
        )
        .required()?;
        assert!(
            matches!(observed, OriginObservation::Observed { ref tips, ref reachable_from } if tips.len() == refs && reachable_from.len() == 5)
        );
        let calls = host.calls.borrow();
        assert_eq!(
            calls.len(),
            9,
            "ref count {refs} must not add sandbox round trips"
        );
        let batches: Vec<_> = calls
            .iter()
            .filter(|spec| {
                spec.args
                    .iter()
                    .any(|arg| arg.starts_with("--batch-check="))
            })
            .collect();
        assert_eq!(batches.len(), 1);
        assert_eq!(
            String::from_utf8_lossy(batches[0].input().required()?)
                .lines()
                .count(),
            refs + 5
        );
        assert!(batches[0].args.len() < 12, "object IDs go through stdin");
        assert!(
            !calls
                .iter()
                .any(|spec| spec.args.iter().any(|arg| arg == "-e" || arg == "fetch"))
        );
        assert_eq!(
            host.clock.now(),
            Moment::since_origin(Duration::from_millis(810))
        );
    }
    Ok(())
}

#[test]
fn refs_and_worktree_heads_that_share_an_object_are_checked_only_once() -> Checked {
    let host = ManyRefs {
        refs: 400,
        duplicate: true,
        clock: ScriptedClock::default(),
        calls: RefCell::new(Vec::new()),
    };
    let id = crate::project::ProjectId::parse("owner/repo")
        .required()?
        .canonical();
    let object = format!("{:040x}", 1);
    let candidates = [
        CommitCandidate::new("refs/heads/a".into(), object.clone(), None),
        CommitCandidate::new("refs/heads/b".into(), object.clone(), None),
    ];
    let observed = observe_read_only(
        &host,
        &SandboxName::derive(&id),
        &SandboxLayout::new(&id),
        &candidates,
        Path::new("/host/repo"),
    )
    .required()?;
    assert!(
        matches!(observed, OriginObservation::Observed { tips, reachable_from } if tips.len() == 400 && reachable_from.len() == 1)
    );
    let calls = host.calls.borrow();
    assert_eq!(calls.len(), 5);
    assert_eq!(
        calls
            .iter()
            .find(|spec| spec.input().is_some())
            .required()?
            .input(),
        Some(format!("{object}\n").as_bytes())
    );
    Ok(())
}

#[test]
fn missing_objects_and_malformed_or_incomplete_batch_replies_never_prove_reachability() -> Checked {
    let scope = ObservationScope {
        sandbox: "sandbox",
        git_dir: "/repo/.git".into(),
    };
    let objects: BTreeSet<&str> = [COMMIT].into_iter().collect();
    let key = "exec -i sandbox -- git --git-dir /repo/.git cat-file --batch-check=%(objectname)";
    for (output, present) in [
        (format!("{COMMIT}\n"), true),
        (format!("{COMMIT} missing\n"), false),
    ] {
        let host = FakeSbx::listing("[]").answering(key, 0, &output);
        assert_eq!(
            objects_present(&host, &scope, &objects).required()?,
            present
        );
    }
    for output in [
        String::new(),
        "garbage\n".into(),
        format!("{COMMIT} ambiguous\n"),
        format!("{COMMIT}\n{COMMIT}\n"),
    ] {
        let host = FakeSbx::listing("[]").answering(key, 0, &output);
        let error = objects_present(&host, &scope, &objects)
            .refused_because("batch output is not a complete answer")?;
        assert_eq!(error.first_id(), Some(ErrorId::ExternalOutputUnparseable));
    }
    let host = FakeSbx::listing("[]").answering(key, 128, "");
    assert_eq!(
        objects_present(&host, &scope, &objects)
            .refused_because("the query failed")?
            .first_id(),
        Some(ErrorId::OriginObservationUnobservable)
    );
    for invalid in [
        "bad\ninput",
        "not-a-sha",
        "gggggggggggggggggggggggggggggggggggggggg",
    ] {
        let host = FakeSbx::listing("[]");
        objects_present(&host, &scope, &[invalid].into_iter().collect())
            .refused_because("one input line must be one full object ID")?;
        assert!(host.calls().is_empty());
    }
    Ok(())
}

#[test]
fn the_real_git_batch_query_needs_no_temporary_directory_or_repository_writes() -> Checked {
    let directory = tempfile::tempdir().required()?;
    crate::testing::repository::git_in(directory.path(), &["init"])?;
    crate::testing::repository::git_in(
        directory.path(),
        &["commit", "--allow-empty", "-m", "initial"],
    )?;
    let commit = crate::testing::repository::git_in(directory.path(), &["rev-parse", "HEAD"])?;
    let git_dir = directory.path().join(".git");
    let scope = ObservationScope {
        sandbox: "sandbox",
        git_dir: git_dir.to_string_lossy().into_owned(),
    };
    let host = ReadOnly(&crate::testing::sandbox::LocalSandbox);
    assert!(objects_present(&host, &scope, &[commit.as_str()].into_iter().collect()).required()?);
    assert!(
        !objects_present(
            &host,
            &scope,
            &["ffffffffffffffffffffffffffffffffffffffff", commit.as_str()]
                .into_iter()
                .collect()
        )
        .required()?
    );
    assert!(
        crate::testing::repository::git_in(directory.path(), &["status", "--porcelain"])?
            .is_empty()
    );
    Ok(())
}
