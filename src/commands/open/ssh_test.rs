use std::collections::VecDeque;

use crate::commands::{Context, add::AddRequest};
use crate::design::prompt::{RecordedScreen, ScriptedKeys};
use crate::design::{PromptUi, RenderingPolicy, Ui};
use crate::diagnostics::{Error, ErrorId, ExitCode, Msg, Result};
use crate::i18n::Locale;
use crate::paths::{PRIVATE_FILE_MODE, PathScope};
use crate::project::ProjectId;
use crate::support::select::{self, ProjectPrompt};
use crate::testing::outcome::{Checked, Refused, Required};
use crate::testing::prompt::ScriptedPrompt;
use crate::testing::provisioning::Bench;
use crate::testing::scripted_clock::ScriptedClock;

use super::fake::SshHost;
use super::run::Prepared;
use super::ssh_repair::SshRepair;
use super::{Args, prepare_connection};

fn built(local: bool) -> Checked<(Bench, SshHost, ProjectId)> {
    let bench = Bench::new()?;
    let host = SshHost::new();
    let request = if local {
        AddRequest {
            repository: crate::repository::RepositoryIdentity::local(
                "/home/user/code/app/.git",
                "app",
            )
            .required()?,
            worktrees: Some(2),
            detach: Some("main".to_string()),
            start_branch: None,
            parent_inside_repository: false,
        }
    } else {
        crate::testing::add_request::request("Example-Org/Example-Repo", Some(2), Some("main"))?
    };
    bench.build(&host.world, &request).required()?;
    let project = ProjectId::parse(&request.repository.display_id()).required()?;
    Ok((bench, host, project))
}

fn prepare(
    bench: &Bench,
    host: &SshHost,
    args: &Args,
    can_prompt: bool,
    prompt: &mut dyn ProjectPrompt,
) -> (Result<Prepared>, String) {
    let clock = ScriptedClock::default();
    let context = Context {
        location: &bench.location,
        workspace_root: bench.workspace_root.path(),
        clock: &clock,
        locale: Locale::En,
        can_prompt,
    };
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let mut ui = Ui::capture(
        Locale::En,
        RenderingPolicy::plain(),
        &mut stdout,
        &mut stderr,
    );
    let result = prepare_connection(args, &context, &bench.config, host, prompt, &mut ui);
    drop(ui);
    (result, String::from_utf8_lossy(&stderr).into_owned())
}

fn args(project: ProjectId) -> Args {
    Args {
        project: Some(project),
        index: Some(1),
    }
}

struct Answers<'a> {
    choices: VecDeque<usize>,
    selection: ScriptedPrompt,
    before: Option<Box<dyn FnMut() + 'a>>,
    fail: bool,
}

impl Answers<'_> {
    fn new(choices: &[usize]) -> Self {
        Self {
            choices: choices.iter().copied().collect(),
            selection: ScriptedPrompt::choosing_worktree(1),
            before: None,
            fail: false,
        }
    }
}

impl ProjectPrompt for Answers<'_> {
    fn select_one(&mut self, heading: &Msg, candidates: &[String]) -> Result<usize> {
        if self.fail {
            return Err(Error::new(
                ErrorId::SelectionUnresolved,
                crate::msg!(
                    "error-selection-unresolved",
                    index = 7,
                    count = candidates.len()
                ),
            ));
        }
        self.selection.select_one(heading, candidates)?;
        if let Some(before) = &mut self.before {
            before();
        }
        Ok(self.choices.pop_front().unwrap_or(0))
    }
    fn select_many(&mut self, heading: &Msg, candidates: &[String]) -> Result<Vec<usize>> {
        self.selection.select_many(heading, candidates)
    }
    fn select_open(
        &mut self,
        heading: &Msg,
        candidates: &[String],
        maximums: &[Option<u32>],
    ) -> Result<(usize, u32)> {
        self.selection.select_open(heading, candidates, maximums)
    }
}

#[test]
fn healthy_ssh_is_probed_without_changing_any_settings() -> Checked {
    let (bench, host, project) = built(false)?;
    let mut prompt = ScriptedPrompt::choosing(1);
    let (prepared, _) = prepare(&bench, &host, &args(project), true, &mut prompt);
    prepared.required()?;
    assert!(host.mutations().is_empty());
    assert!(prompt.headings.borrow().is_empty());
    let calls = host.calls.borrow();
    let probe = calls
        .iter()
        .find(|spec| spec.program == "ssh" && spec.args.first().is_some_and(|arg| arg == "-n"))
        .required()?;
    assert_eq!(probe.timeout, crate::boundary::host::TimeoutClass::Probe);
    assert_eq!(
        probe.env,
        crate::boundary::host::EnvPolicy::InheritWithoutSshAgent
    );
    for option in [
        "BatchMode=yes",
        "ForwardAgent=no",
        "ClearAllForwardings=yes",
        "ControlPath=none",
        "StrictHostKeyChecking=yes",
    ] {
        assert!(probe.args.iter().any(|arg| arg == option));
    }
    Ok(())
}

#[test]
fn each_required_repair_is_approved_and_the_project_and_index_are_retained() -> Checked {
    let (bench, host, _) = built(true)?;
    host.configured.set(false);
    host.enabled.set(false);
    host.endpoint.set(false);
    let mut prompt = Answers::new(&[1, 1, 1]);
    let (prepared, shown) = prepare(
        &bench,
        &host,
        &Args {
            project: None,
            index: None,
        },
        true,
        &mut prompt,
    );
    let prepared = prepared.required()?;
    assert_eq!(prepared.project, "local/app");
    assert!(prepared.working_directory.ends_with("app.tree-1"));
    assert_eq!(
        host.mutations(),
        [
            "sbx setup ssh",
            "sbx settings set feature.ssh true",
            "sbx daemon restart"
        ]
    );
    assert_eq!(
        prompt
            .selection
            .headings
            .borrow()
            .iter()
            .filter(|id| **id == "select-open-heading")
            .count(),
        1
    );
    assert!(shown.contains("other sandboxes"), "{shown}");
    let calls = host.invocations();
    let restart = calls
        .iter()
        .position(|call| call == "sbx daemon restart")
        .required()?;
    assert!(
        calls
            .iter()
            .skip(restart + 1)
            .any(|call| call.starts_with("ssh -n "))
    );
    assert!(!calls.iter().any(|call| call.starts_with("ssh -t ")));
    Ok(())
}

#[test]
fn endpoint_unavailability_restarts_only_after_approval_and_rechecks_the_interior() -> Checked {
    let (bench, host, project) = built(false)?;
    host.endpoint.set(false);
    let (prepared, _) = prepare(&bench, &host, &args(project), true, &mut Answers::new(&[1]));
    prepared.required()?;
    assert_eq!(host.mutations(), ["sbx daemon restart"]);
    assert_eq!(
        host.invocations()
            .iter()
            .filter(|call| call.contains("# sbxm inside check"))
            .count(),
        2
    );
    Ok(())
}

#[test]
fn noninteractive_runs_declines_and_cancellation_leave_ssh_unchanged() -> Checked {
    let (bench, host, project) = built(false)?;
    host.enabled.set(false);
    for (interactive, mut prompt) in [
        (false, ScriptedPrompt::choosing(1)),
        (true, ScriptedPrompt::choosing(0)),
        (true, ScriptedPrompt::canceling()),
    ] {
        let (result, _) = prepare(
            &bench,
            &host,
            &args(project.clone()),
            interactive,
            &mut prompt,
        );
        let error = result.refused_because("SSH remains disabled")?;
        assert_eq!(error.first_id(), Some(ErrorId::RemoteSshDisabled));
        if !interactive {
            assert!(prompt.headings.borrow().is_empty());
        }
        assert!(host.mutations().is_empty());
    }
    Ok(())
}

#[test]
fn refusing_the_restart_retains_the_approved_setting_without_starting_a_session() -> Checked {
    let (bench, host, project) = built(false)?;
    host.enabled.set(false);
    host.endpoint.set(false);
    let (result, _) = prepare(
        &bench,
        &host,
        &args(project),
        true,
        &mut Answers::new(&[1, 0]),
    );
    assert_eq!(
        result
            .refused_because("the restart was declined")?
            .first_id(),
        Some(ErrorId::RemoteSshRestartRequired)
    );
    assert_eq!(host.mutations(), ["sbx settings set feature.ssh true"]);
    assert!(host.enabled.get());
    assert!(
        !host
            .invocations()
            .iter()
            .any(|call| call.starts_with("ssh -t "))
    );
    Ok(())
}

#[test]
fn administrator_gate_and_non_overridable_sources_are_never_changed() -> Checked {
    let (bench, host, project) = built(false)?;
    for (allowed, enabled, source, locked) in [
        (false, true, "override", false),
        (true, false, "envvar", false),
        (true, false, "admin", false),
        (true, false, "override", true),
        (true, false, "unknown", false),
    ] {
        host.allowed.set(allowed);
        host.enabled.set(enabled);
        host.source.replace(source.to_string());
        host.locked.set(locked);
        let mut prompt = ScriptedPrompt::choosing(1);
        let (result, _) = prepare(&bench, &host, &args(project.clone()), true, &mut prompt);
        assert_eq!(
            result
                .refused_because("the constraint is retained")?
                .first_id(),
            Some(ErrorId::RemoteSshRestricted)
        );
        assert!(prompt.headings.borrow().is_empty());
        assert!(host.mutations().is_empty());
    }
    Ok(())
}

#[test]
fn a_rejected_repair_does_not_restart_or_bypass_the_policy() -> Checked {
    let (bench, host, project) = built(false)?;
    host.enabled.set(false);
    host.refuse.replace(Some("settings set"));
    let (result, _) = prepare(&bench, &host, &args(project), true, &mut Answers::new(&[1]));
    let error = result.refused_because("administrator policy rejected the operation")?;
    assert_eq!(error.first_id(), Some(ErrorId::RemoteSshRepairFailed));
    assert!(
        error.diagnostics()[0]
            .external
            .as_ref()
            .required()?
            .stderr_text()
            .contains("administrator policy")
    );
    assert_eq!(host.mutations(), ["sbx settings set feature.ssh true"]);
    assert!(!host.enabled.get());
    Ok(())
}

#[test]
fn a_successful_command_that_does_not_fix_the_state_is_not_repeated() -> Checked {
    let (bench, host, project) = built(false)?;
    host.endpoint.set(false);
    host.stubborn.set(true);
    let mut prompt = Answers::new(&[1, 1]);
    let (result, _) = prepare(&bench, &host, &args(project), true, &mut prompt);
    assert_eq!(
        result
            .refused_because("the endpoint is still unavailable")?
            .first_id(),
        Some(ErrorId::RemoteSshUnavailable)
    );
    assert_eq!(host.mutations(), ["sbx daemon restart"]);
    assert_eq!(prompt.selection.headings.borrow().len(), 1);
    Ok(())
}

#[test]
fn an_unknown_ssh_failure_preserves_its_stderr_and_offers_no_guessed_repair() -> Checked {
    let (bench, host, project) = built(false)?;
    host.probe_stderr
        .replace(Some("Host key verification failed."));
    let mut prompt = ScriptedPrompt::choosing(1);
    let (result, _) = prepare(&bench, &host, &args(project), true, &mut prompt);
    let error = result.refused_because("host key verification must be resolved")?;
    assert_eq!(error.first_id(), Some(ErrorId::RemoteSshConnectionFailed));
    assert!(
        error.diagnostics()[0]
            .external
            .as_ref()
            .required()?
            .stderr_text()
            .contains("Host key verification failed")
    );
    assert!(host.mutations().is_empty());
    assert!(prompt.headings.borrow().is_empty());
    Ok(())
}

#[test]
fn settings_fixed_while_the_user_was_answering_are_left_alone() -> Checked {
    let (bench, host, project) = built(false)?;
    host.enabled.set(false);
    let mut prompt = Answers::new(&[1]);
    prompt.before = Some(Box::new(|| host.enabled.set(true)));
    let (result, _) = prepare(&bench, &host, &args(project), true, &mut prompt);
    result.required()?;
    assert!(host.mutations().is_empty());
    Ok(())
}

#[test]
fn repair_questions_hold_neither_the_project_lock_nor_the_session_lease() -> Checked {
    let (bench, host, project) = built(false)?;
    host.endpoint.set(false);
    let candidate = select::find(&bench.location, &project).required()?;
    let unlocked = std::cell::Cell::new(false);
    let mut prompt = Answers::new(&[1]);
    prompt.before = Some(Box::new(|| {
        let project_available = candidate
            .clone()
            .lock_within(std::time::Duration::ZERO)
            .is_ok();
        let lease_available = crate::paths::acquire_exclusive_lock(
            &candidate.paths.session_lease_file(),
            std::time::Duration::ZERO,
            PRIVATE_FILE_MODE,
            PathScope::ProjectPath,
        )
        .is_ok();
        unlocked.set(project_available && lease_available);
    }));
    prepare(&bench, &host, &args(project), true, &mut prompt)
        .0
        .required()?;
    drop(prompt);
    assert!(unlocked.get());
    Ok(())
}

#[test]
fn settings_that_hot_reload_do_not_restart_the_daemon() -> Checked {
    let (bench, host, project) = built(false)?;
    host.enabled.set(false);
    host.requires_restart.set(false);
    prepare(&bench, &host, &args(project), true, &mut Answers::new(&[1]))
        .0
        .required()?;
    assert_eq!(host.mutations(), ["sbx settings set feature.ssh true"]);
    Ok(())
}

fn exec(bench: &Bench, host: &SshHost, project: ProjectId) -> (ExitCode, String) {
    let clock = ScriptedClock::default();
    let context = Context {
        location: &bench.location,
        workspace_root: bench.workspace_root.path(),
        clock: &clock,
        locale: Locale::En,
        can_prompt: false,
    };
    let policy = RenderingPolicy::plain();
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    let mut ui = Ui::capture(Locale::En, policy, &mut stdout, &mut stderr);
    let mut prompt = PromptUi::new(
        Locale::En,
        policy.stderr,
        Box::new(ScriptedKeys::confirming()),
        Box::new(RecordedScreen::new()),
    );
    let code = super::exec(&args(project), &context, &mut ui, host, &mut prompt);
    drop(ui);
    (code, String::from_utf8_lossy(&stderr).into_owned())
}

#[test]
fn failed_preflight_does_not_save_or_start_a_session_and_reports_commit_preservation() -> Checked {
    let (bench, host, project) = built(true)?;
    host.endpoint.set(false);
    let before = bench.stored("local/app")?;
    let (code, shown) = exec(&bench, &host, project);
    assert_eq!(code, ExitCode::Failure);
    assert!(shown.contains("remote-ssh-unavailable"), "{shown}");
    assert!(
        shown.contains("Unsynchronized commits may exist only in the sandbox"),
        "{shown}"
    );
    assert!(shown.contains("sbxm sync local/app"), "{shown}");
    assert!(shown.contains("sbxm open local/app -i 1"), "{shown}");
    assert!(!shown.contains("sandbox-repository-unreadable"), "{shown}");
    assert!(!shown.contains("Connecting to"), "{shown}");
    assert!(
        !host
            .invocations()
            .iter()
            .any(|call| call.starts_with("ssh -t ")
                || call.contains(crate::support::host_sync::PLACE_SAVE_REFS))
    );
    assert_eq!(bench.stored("local/app")?, before);
    Ok(())
}

#[test]
fn a_session_failure_keeps_the_save_opportunity_and_labels_its_failure_as_secondary() -> Checked {
    let (bench, host, project) = built(true)?;
    host.session_fails.set(true);
    host.save_fails.set(true);
    let (code, shown) = exec(&bench, &host, project);
    assert_eq!(code, ExitCode::Failure);
    let primary = shown.find("external-command-failed").required()?;
    let secondary = shown.find("This is a secondary failure").required()?;
    assert!(primary < secondary, "{shown}");
    assert!(shown.contains("sandbox-repository-unreadable"), "{shown}");
    assert!(shown.contains("still only in the sandbox"), "{shown}");
    assert!(
        host.invocations()
            .iter()
            .any(|call| call.contains(crate::support::host_sync::PLACE_SAVE_REFS))
    );
    Ok(())
}

#[test]
fn a_recheck_that_finds_a_different_constraint_stops_before_the_approved_change() -> Checked {
    let host = SshHost::new();
    host.allowed.set(false);
    let error = SshRepair::Enable
        .execute(&host, &ScriptedClock::default(), "sandbox", false)
        .refused_because("the administrator gate changed")?;
    assert_eq!(error.first_id(), Some(ErrorId::RemoteSshRestricted));
    assert!(host.mutations().is_empty());
    assert!(
        !SshRepair::Setup
            .execute(&SshHost::new(), &ScriptedClock::default(), "sandbox", false)
            .required()?
    );
    Ok(())
}

#[test]
fn invalid_or_failed_prompt_answers_never_authorize_a_repair() -> Checked {
    let (bench, host, project) = built(false)?;
    host.enabled.set(false);
    for fail in [true, false] {
        let mut prompt = Answers::new(&[7]);
        prompt.fail = fail;
        let error = prepare(&bench, &host, &args(project.clone()), true, &mut prompt)
            .0
            .refused_because("no approval was given")?;
        assert_eq!(error.first_id(), Some(ErrorId::SelectionUnresolved));
        assert!(host.mutations().is_empty());
    }
    Ok(())
}

#[test]
fn an_endpoint_fixed_while_confirmation_was_pending_is_not_restarted() -> Checked {
    let (bench, host, project) = built(false)?;
    host.endpoint.set(false);
    let mut prompt = Answers::new(&[1]);
    prompt.before = Some(Box::new(|| host.endpoint.set(true)));
    prepare(&bench, &host, &args(project), true, &mut prompt)
        .0
        .required()?;
    assert!(host.mutations().is_empty());
    Ok(())
}

#[test]
fn unobservable_settings_and_administrator_constraints_are_not_treated_as_disabled() -> Checked {
    let (bench, host, project) = built(false)?;
    for needle in [
        "settings get feature.ssh",
        "settings get platform.allowExperimentalFeatures",
    ] {
        host.refuse.replace(Some(needle));
        let mut prompt = Answers::new(&[1]);
        let error = prepare(&bench, &host, &args(project.clone()), true, &mut prompt)
            .0
            .refused_because("the effective setting is unknown")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::RemoteSshSettingUnobservable)
        );
        assert!(error.diagnostics()[0].external.is_some());
        assert!(host.mutations().is_empty());
        assert!(prompt.selection.headings.borrow().is_empty());
    }
    host.refuse.replace(None);
    for needle in [
        "settings get feature.ssh",
        "settings get platform.allowExperimentalFeatures",
    ] {
        host.timeout.replace(Some(needle));
        let error = prepare(
            &bench,
            &host,
            &args(project.clone()),
            false,
            &mut Answers::new(&[]),
        )
        .0
        .refused_because("settings timed out")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::RemoteSshSettingUnobservable)
        );
        assert!(error.diagnostics()[0].external.is_none());
    }
    host.timeout.replace(None);
    host.source.replace(String::new());
    assert_eq!(
        prepare(&bench, &host, &args(project), false, &mut Answers::new(&[]))
            .0
            .refused_because("the record has no valid source")?
            .first_id(),
        Some(ErrorId::RemoteSshSettingUnobservable)
    );
    Ok(())
}

#[test]
fn a_forbidden_endpoint_does_not_offer_to_rewrite_missing_client_configuration() -> Checked {
    let (bench, host, project) = built(false)?;
    host.configured.set(false);
    host.allowed.set(false);
    let mut prompt = Answers::new(&[1]);
    let error = prepare(&bench, &host, &args(project), true, &mut prompt)
        .0
        .refused_because("SSH is prohibited")?;
    assert_eq!(error.first_id(), Some(ErrorId::RemoteSshRestricted));
    assert!(prompt.selection.headings.borrow().is_empty());
    assert!(host.mutations().is_empty());
    Ok(())
}

#[test]
fn a_probe_timeout_and_a_repair_timeout_remain_distinct_and_stop_safely() -> Checked {
    let (bench, host, project) = built(true)?;
    host.timeout.replace(Some("ssh -n"));
    let error = prepare(
        &bench,
        &host,
        &args(project.clone()),
        false,
        &mut Answers::new(&[]),
    )
    .0
    .refused_because("the probe timed out")?;
    assert_eq!(error.first_id(), Some(ErrorId::RemoteSshConnectionFailed));
    assert!(host.mutations().is_empty());
    host.timeout.replace(Some("sbx daemon restart"));
    host.endpoint.set(false);
    let error = prepare(&bench, &host, &args(project), true, &mut Answers::new(&[1]))
        .0
        .refused_because("the approved repair timed out")?;
    assert_eq!(error.first_id(), Some(ErrorId::RemoteSshRepairFailed));
    assert_eq!(host.mutations(), ["sbx daemon restart"]);
    Ok(())
}

#[test]
fn disabled_ssh_stops_a_new_local_project_before_creating_any_sandbox() -> Checked {
    let bench = Bench::new()?;
    let host = SshHost::new();
    let request = AddRequest {
        repository: crate::repository::RepositoryIdentity::local("/home/user/code/app/.git", "app")
            .required()?,
        worktrees: None,
        detach: None,
        start_branch: Some("main".to_string()),
        parent_inside_repository: false,
    };
    let project = bench.register(&host.world, &request).required()?;
    host.enabled.set(false);
    let error = prepare(&bench, &host, &args(project), false, &mut Answers::new(&[]))
        .0
        .refused_because("SSH is needed for the first push")?;
    assert_eq!(error.first_id(), Some(ErrorId::RemoteSshDisabled));
    assert!(host.world.sandboxes.borrow().is_empty());
    assert!(
        !host
            .invocations()
            .iter()
            .any(|call| call.starts_with("ssh -n ")
                || call.starts_with("docker build ")
                || call.starts_with("sbx create "))
    );
    Ok(())
}
