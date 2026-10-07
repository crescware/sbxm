use std::cell::Cell;
use std::io::BufWriter;
use std::rc::Rc;

use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::design::{PromptUi, RenderingPolicy, Ui};
use crate::diagnostics::ExitCode;
use crate::i18n::Locale;
use crate::testing::add_request::{project_of, request};
use crate::testing::command::ScriptedWriter;
use crate::testing::host::ChangingBefore;
use crate::testing::outcome::{Checked, Required};
use crate::testing::provisioning::{Bench, World};
use crate::testing::scripted_clock::ScriptedClock;

use super::{Context, apply, open, repair};

#[derive(Clone, Copy)]
enum Entry {
    Open,
    Repair,
    Apply,
    ApplyAll,
}

#[test]
fn open_repair_and_apply_flush_the_same_inspection_phases_before_waiting() -> Checked {
    for (needle, en, ja) in [
        (
            "--is-bare-repository",
            "Checking the repository inside the sandbox.",
            "Sandbox内のrepositoryを確認します",
        ),
        (
            "fsck --connectivity-only",
            "object connectivity",
            "objectの接続性を確認します",
        ),
        (
            "--git-common-dir",
            "shared repository membership",
            "共有repositoryに属することを確認します",
        ),
        (
            "rev-parse HEAD",
            "HEAD and upstream",
            "HEADとupstreamを確認します",
        ),
    ] {
        for entry in [Entry::Open, Entry::Repair, Entry::Apply] {
            for (locale, expected) in [(Locale::En, en), (Locale::Ja, ja)] {
                check_entry(entry, needle, expected, locale)?;
            }
        }
    }
    for entry in [Entry::Open, Entry::Repair, Entry::Apply, Entry::ApplyAll] {
        for (locale, expected) in [(Locale::En, "Checking file 1/1"), (Locale::Ja, "file 1/1")] {
            check_entry(entry, "sha256sum", expected, locale)?;
        }
    }
    Ok(())
}

fn check_entry(
    entry: Entry,
    needle: &'static str,
    expected: &'static str,
    locale: Locale,
) -> Checked {
    let bench = Bench::new()?;
    let world = World::new();
    let request = request("Example-Org/Example-Repo", Some(1), None)?;
    bench.build(&world, &request).required()?;
    for declaration in &bench.config.files {
        crate::config::save_file_declaration(&bench.location, declaration).required()?;
    }
    let project = project_of(&request)?;
    let clock = ScriptedClock::default();
    let policy = RenderingPolicy::plain();
    let stdout = ScriptedWriter::accepting_all();
    let stderr = ScriptedWriter::accepting_all();
    let visible_stdout = stdout.written();
    let visible_stderr = stderr.written();
    let observed = Rc::new(Cell::new(false));
    let observed_by_host = observed.clone();
    let host = ChangingBefore::new(world, needle, move || {
        let progress = String::from_utf8_lossy(&visible_stderr.borrow()).into_owned();
        assert!(progress.contains(expected), "{needle}: {progress}");
        if needle == "rev-parse HEAD" || needle == "--git-common-dir" {
            assert!(
                progress.contains("worktree 1/1") || progress.contains("worktree 1/2"),
                "{progress}"
            );
            assert!(progress.contains("example-repo.tree-"), "{progress}");
        }
        assert!(
            visible_stdout.borrow().is_empty(),
            "the result or plan comes later"
        );
        observed_by_host.set(true);
    });
    let mut ui = Ui::new(
        locale,
        policy,
        BufWriter::new(stdout),
        BufWriter::new(stderr),
    );
    let mut prompt = PromptUi::new(
        locale,
        policy.stderr,
        Box::new(ScriptedKeys::choosing(0)),
        Box::new(RecordedScreen::new()),
    );
    let context = Context {
        location: &bench.location,
        workspace_root: bench.workspace_root.path(),
        clock: &clock,
        locale,
        can_prompt: false,
    };
    match entry {
        Entry::Open => {
            open::run::prepare(
                &bench.location,
                &bench.config,
                Some(&project),
                None,
                &host,
                &mut prompt,
                bench.workspace_root.path(),
                crate::support::inventory::Poll::standard(&clock),
                &mut ui,
            )
            .required()?;
        }
        Entry::Repair => {
            assert_eq!(
                repair::exec(Some(&project), &context, &mut ui, &host, &mut prompt),
                ExitCode::Success
            );
        }
        Entry::Apply | Entry::ApplyAll => {
            let files = needle == "sha256sum";
            let args = apply::Args {
                project: Some(project),
                all: matches!(entry, Entry::ApplyAll),
                files,
                force: false,
                worktrees: (!files).then_some(2),
            };
            assert_eq!(
                apply::exec(&args, &context, &mut ui, &host, &mut prompt),
                ExitCode::Success
            );
        }
    }
    assert!(
        observed.get(),
        "the inspected command was not reached: {needle}"
    );
    Ok(())
}
