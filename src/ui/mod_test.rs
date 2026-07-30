use super::*;

use crate::error::{Diagnostic, ErrorId};

/// 2つのstreamを別々に受け取る。統合した順序をtestの前提にしない。
struct Streams {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

impl Streams {
    fn capture(policy: OutputPolicy, act: impl FnOnce(&mut Ui<'_>)) -> Streams {
        let mut stdout: Vec<u8> = Vec::new();
        let mut stderr: Vec<u8> = Vec::new();
        {
            let mut ui = Ui::capture(Locale::En, policy, &mut stdout, &mut stderr);
            act(&mut ui);
        }
        Streams {
            stdout: stdout.clone(),
            stderr: stderr.clone(),
        }
    }

    fn out(&self) -> String {
        String::from_utf8(self.stdout.clone()).expect("UTF-8")
    }

    fn err(&self) -> String {
        String::from_utf8(self.stderr.clone()).expect("UTF-8")
    }
}

fn summary() -> Document {
    Document::new().summary(crate::msg!("add-registered", project = "owner/alpha"))
}

#[test]
fn results_go_to_stdout_and_everything_else_to_stderr() {
    let streams = Streams::capture(OutputPolicy::plain(), |ui| {
        ui.progress(crate::msg!("progress-creating-sandbox"));
        ui.warning(&Warning::text(crate::msg!("destroy-force-notice")));
        ui.error(&crate::error::Error::new(
            ErrorId::DockerUnreachable,
            crate::msg!("error-docker-unreachable", detail = "no answer"),
        ));
        ui.stdout(&summary());
    });

    assert!(streams.out().starts_with("\u{2713} "), "{}", streams.out());
    assert!(!streams.out().contains("error:"), "{}", streams.out());
    let err = streams.err();
    assert!(err.contains("\u{2192} "), "{err}");
    assert!(err.contains("! Warning: "), "{err}");
    assert!(err.contains("\u{d7} error: "), "{err}");
}

#[test]
fn the_two_streams_carry_their_own_color_decision() {
    // stdoutだけをpipeした場合、結果はplain textで診断は色付きになる。
    let policy = OutputPolicy {
        stdout: policy::StreamPolicy::plain(),
        stderr: policy::StreamPolicy::colored(),
    };
    let streams = Streams::capture(policy, |ui| {
        ui.stdout(&summary());
        ui.warning(&Warning::text(crate::msg!("destroy-force-notice")));
    });

    assert!(!streams.stdout.contains(&0x1b), "{:?}", streams.out());
    assert!(streams.stderr.contains(&0x1b), "{:?}", streams.err());
}

#[test]
fn separate_calls_to_the_same_stream_are_still_one_blank_line_apart() {
    // blockの間隔はdocumentごとではなくstreamごとに数える。
    let streams = Streams::capture(OutputPolicy::plain(), |ui| {
        ui.stdout(&summary());
        ui.stdout(&Document::new().note(crate::msg!("files-secret-hint")));
    });
    let out = streams.out();
    assert!(!out.contains("\n\n\n"), "{out:?}");
    assert_eq!(
        out.lines().filter(|line| line.is_empty()).count(),
        1,
        "{out:?}"
    );
}

#[test]
fn consecutive_progress_stays_together_and_the_summary_after_it_does_not() {
    let streams = Streams::capture(OutputPolicy::plain(), |ui| {
        ui.progress(crate::msg!("progress-creating-sandbox"));
        ui.progress(crate::msg!("progress-starting-sandbox"));
        ui.stderr(&Document::new().summary(crate::msg!("destroy-done", project = "owner/alpha")));
    });
    let err = streams.err();
    let lines: Vec<&str> = err.lines().collect();
    assert_eq!(lines.len(), 4, "{lines:?}");
    assert_eq!(
        lines[2], "",
        "the summary is one blank line below: {lines:?}"
    );
}

#[test]
fn a_warning_with_a_follow_up_keeps_the_command_on_its_own_line() {
    let warning = Warning::text(crate::msg!(
        "warning-dockerfile-changed-during-build",
        project = "owner/alpha"
    ))
    .explain(crate::msg!("guidance-apply-current-dockerfile"))
    .try_run("sbxm rebuild owner/alpha");

    let streams = Streams::capture(OutputPolicy::plain(), |ui| ui.warning(&warning));
    let err = streams.err();
    let lines: Vec<&str> = err.lines().collect();
    let index = lines
        .iter()
        .position(|line| *line == "sbxm rebuild owner/alpha")
        .expect("the follow-up is its own line");
    assert_eq!(lines[index - 1], "", "{err:?}");
    assert!(lines[0].starts_with("! Warning: "), "{err:?}");
}

#[test]
fn a_cancel_reports_nothing_at_all() {
    // Ctrl-CとEscは何も変更していない。画面にも何も残さない。
    let streams = Streams::capture(OutputPolicy::plain(), |ui| {
        ui.error(&crate::error::Error::Canceled);
    });
    assert!(streams.err().is_empty(), "{:?}", streams.err());
    assert!(streams.out().is_empty(), "{:?}", streams.out());
}

#[test]
fn every_diagnostic_of_one_error_is_reported() {
    let error = crate::error::Error::many(vec![
        Diagnostic::new(
            ErrorId::ConfigMissing,
            crate::msg!("error-config-missing", path = "/x"),
        ),
        Diagnostic::new(
            ErrorId::DockerUnreachable,
            crate::msg!("error-docker-unreachable", detail = "no answer"),
        ),
    ]);
    let streams = Streams::capture(OutputPolicy::plain(), |ui| ui.error(&error));
    assert_eq!(streams.err().matches("\u{d7} error:").count(), 2);
}

#[test]
fn the_locale_can_be_switched_once_the_configuration_declares_one() {
    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    {
        let mut ui = Ui::capture(Locale::En, OutputPolicy::plain(), &mut stdout, &mut stderr);
        assert_eq!(ui.locale(), Locale::En);
        ui.set_locale(Locale::Ja);
        assert_eq!(ui.locale(), Locale::Ja);
        ui.stdout(&Document::new().note(crate::msg!("files-secret-hint")));
    }
    let out = String::from_utf8(stdout).expect("UTF-8");
    assert!(out.starts_with("! 注記 (Note): "), "{out:?}");
}

#[test]
fn a_progress_sink_that_reports_nothing_is_still_a_valid_sink() {
    let mut silent = SilentProgress;
    silent.step(crate::msg!("progress-creating-sandbox"));
}

#[test]
fn the_prompt_inherits_the_stderr_policy_without_borrowing_the_ui() {
    // 同じworkflowが進捗の報告とpromptの両方を必要としても借用が衝突しない。
    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    let mut ui = Ui::capture(Locale::En, OutputPolicy::plain(), &mut stdout, &mut stderr);
    let mut prompt = ui.prompt();
    ui.step(crate::msg!("progress-creating-sandbox"));
    let _ = &mut prompt;
}
