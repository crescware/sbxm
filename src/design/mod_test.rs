use crate::i18n::Locale;

use crate::testing::outcome::{Checked, Required};

use super::*;

use crate::diagnostics::{Diagnostic, ErrorId};

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

    fn out(&self) -> Checked<String> {
        String::from_utf8(self.stdout.clone()).required_because("UTF-8")
    }

    fn err(&self) -> Checked<String> {
        String::from_utf8(self.stderr.clone()).required_because("UTF-8")
    }
}

fn summary() -> Document {
    Document::new().summary(crate::msg!("add-registered", project = "owner/alpha"))
}

#[test]
fn results_go_to_stdout_and_everything_else_to_stderr() -> Checked {
    let streams = Streams::capture(OutputPolicy::plain(), |ui| {
        ui.progress(crate::msg!("progress-creating-sandbox"));
        ui.warning(&Warning::text(crate::msg!("destroy-force-notice")));
        ui.error(&crate::diagnostics::Error::new(
            ErrorId::DockerUnreachable,
            crate::msg!("error-docker-unreachable"),
        ));
        ui.stdout(&summary());
    });

    let out = streams.out()?;
    assert!(out.starts_with("\u{2713} "), "{out}");
    assert!(!out.contains("error:"), "{out}");
    let err = streams.err()?;
    assert!(err.contains("\u{2192} "), "{err}");
    assert!(err.contains("! Warning: "), "{err}");
    assert!(err.contains("\u{d7} error: "), "{err}");
    Ok(())
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
fn separate_calls_to_the_same_stream_are_still_one_blank_line_apart() -> Checked {
    // blockの間隔はdocumentごとではなくstreamごとに数える。
    let streams = Streams::capture(OutputPolicy::plain(), |ui| {
        ui.stdout(&summary());
        ui.stdout(&Document::new().note(crate::msg!("files-secret-hint")));
    });
    let out = streams.out()?;
    assert!(!out.contains("\n\n\n"), "{out:?}");
    assert_eq!(
        out.lines().filter(|line| line.is_empty()).count(),
        1,
        "{out:?}"
    );
    Ok(())
}

#[test]
fn consecutive_progress_stays_together_and_the_summary_after_it_does_not() -> Checked {
    let streams = Streams::capture(OutputPolicy::plain(), |ui| {
        ui.progress(crate::msg!("progress-creating-sandbox"));
        ui.progress(crate::msg!("progress-starting-sandbox"));
        ui.stderr(&Document::new().summary(crate::msg!("destroy-done", project = "owner/alpha")));
    });
    let err = streams.err()?;
    let lines: Vec<&str> = err.lines().collect();
    assert_eq!(lines.len(), 4, "{lines:?}");
    assert_eq!(
        lines[2], "",
        "the summary is one blank line below: {lines:?}"
    );
    Ok(())
}

#[test]
fn a_warning_with_a_follow_up_keeps_the_command_on_its_own_line() -> Checked {
    let warning = Warning::text(crate::msg!(
        "warning-dockerfile-changed-during-build",
        project = "owner/alpha"
    ))
    .explain(crate::msg!("guidance-apply-current-dockerfile"))
    .try_run("sbxm rebuild owner/alpha");

    let streams = Streams::capture(OutputPolicy::plain(), |ui| ui.warning(&warning));
    let err = streams.err()?;
    let lines: Vec<&str> = err.lines().collect();
    let index = lines
        .iter()
        .position(|line| *line == "  sbxm rebuild owner/alpha")
        .required_because("the follow-up is its own line")?;
    assert_eq!(lines[index - 1], "", "{err:?}");
    assert!(lines[0].starts_with("! Warning: "), "{err:?}");
    Ok(())
}

#[test]
fn a_cancel_reports_nothing_at_all() -> Checked {
    // Ctrl-CとEscは何も変更していない。画面にも何も残さない。
    let streams = Streams::capture(OutputPolicy::plain(), |ui| {
        ui.error(&crate::diagnostics::Error::Canceled);
    });
    assert!(streams.err()?.is_empty(), "{:?}", streams.err());
    assert!(streams.out()?.is_empty(), "{:?}", streams.out());
    Ok(())
}

#[test]
fn every_diagnostic_of_one_error_is_reported() -> Checked {
    let error = crate::diagnostics::Error::many(vec![
        Diagnostic::new(
            ErrorId::ConfigUnreadable,
            crate::msg!("error-config-unreadable"),
        ),
        Diagnostic::new(
            ErrorId::DockerUnreachable,
            crate::msg!("error-docker-unreachable"),
        ),
    ]);
    let streams = Streams::capture(OutputPolicy::plain(), |ui| ui.error(&error));
    assert_eq!(streams.err()?.matches("\u{d7} error:").count(), 2);
    Ok(())
}

#[test]
fn the_locale_can_be_switched_once_the_configuration_declares_one() -> Checked {
    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    {
        let mut ui = Ui::capture(Locale::En, OutputPolicy::plain(), &mut stdout, &mut stderr);
        assert_eq!(ui.locale(), Locale::En);
        ui.set_locale(Locale::Ja);
        assert_eq!(ui.locale(), Locale::Ja);
        ui.stdout(&Document::new().note(crate::msg!("files-secret-hint")));
    }
    let out = String::from_utf8(stdout).required_because("UTF-8")?;
    assert!(out.starts_with("! 注記 (Note): "), "{out:?}");
    Ok(())
}

#[test]
fn a_progress_sink_that_reports_nothing_is_still_a_valid_sink() {
    let mut silent = SilentProgress;
    silent.step(crate::msg!("progress-creating-sandbox"));
}

#[test]
fn an_external_tool_is_separated_from_the_sbxm_lines_by_one_blank_line_on_both_sides() -> Checked {
    let streams = Streams::capture(OutputPolicy::plain(), |ui| {
        ui.progress(crate::msg!("progress-cloning-host"));
        ui.relay(b"Cloning into 'example'...\n");
        ui.relay(b"Receiving objects: 100%, done.\n");
        ui.finished();
        ui.progress(crate::msg!("progress-building-image"));
    });

    let err = streams.err()?;
    let lines: Vec<&str> = err.lines().collect();
    assert_eq!(
        lines,
        vec![
            "\u{2192} Cloning the repository onto this host. For a large repository this can take several minutes.",
            "",
            "Cloning into 'example'...",
            "Receiving objects: 100%, done.",
            "",
            "\u{2192} Building the sandbox image. The first build downloads packages and can take several minutes.",
        ],
        "one blank line stands on each side of what the tool said: {err:?}"
    );
    Ok(())
}

#[test]
fn an_external_tool_that_says_nothing_leaves_the_progress_lines_together() -> Checked {
    let streams = Streams::capture(OutputPolicy::plain(), |ui| {
        ui.progress(crate::msg!("progress-starting-sandbox"));
        ui.finished();
        ui.progress(crate::msg!("progress-building-image"));
    });

    let err = streams.err()?;
    assert!(
        !err.contains("\n\n"),
        "a tool that wrote nothing has no boundary to mark: {err:?}"
    );
    Ok(())
}

#[test]
fn a_tool_that_ends_without_a_newline_still_gets_its_own_line() -> Checked {
    // 進捗表示は復帰文字で行を上書きするため、最後の1行が改行なしで終わることがある。
    let streams = Streams::capture(OutputPolicy::plain(), |ui| {
        ui.progress(crate::msg!("progress-cloning-host"));
        ui.relay(b"Receiving objects:  50%\rReceiving objects: 100%");
        ui.finished();
        ui.stderr(&Document::new().summary(crate::msg!("add-registered", project = "owner/alpha")));
    });

    let err = streams.err()?;
    assert!(
        err.contains("Receiving objects: 100%\n\n\u{2713} "),
        "the summary starts on its own line, one blank away: {err:?}"
    );
    Ok(())
}

#[test]
fn a_handed_over_terminal_is_opened_with_a_blank_line_after_the_result() -> Checked {
    // 中継しない相手が何を書くかは観測できないため、境界は渡す前に置く。
    let streams = Streams::capture(OutputPolicy::plain(), |ui| {
        ui.stderr(&summary());
        ui.hand_over();
        ui.finished();
        ui.progress(crate::msg!("progress-starting-sandbox"));
    });

    let err = streams.err()?;
    assert!(
        err.starts_with("\u{2713} owner/alpha is now managed.\n\n"),
        "the terminal is handed over one blank line below the result: {err:?}"
    );
    Ok(())
}

#[test]
fn a_result_on_stdout_and_a_relay_on_stderr_share_one_boundary() -> Checked {
    // 端末では2つのstreamが同じ場所へ出る。境界の空行が2つ並ばない。
    let streams = Streams::capture(OutputPolicy::plain(), |ui| {
        ui.stdout(&summary());
        ui.progress(crate::msg!("progress-starting-sandbox"));
        ui.relay(b"Sandbox started successfully\n");
        ui.finished();
    });

    let err = streams.err()?;
    assert!(
        err.ends_with("\n\nSandbox started successfully\n"),
        "the relay opens with exactly one blank line: {err:?}"
    );
    assert!(
        !streams.out()?.ends_with("\n\n"),
        "the boundary is written once, next to what follows it"
    );
    Ok(())
}
