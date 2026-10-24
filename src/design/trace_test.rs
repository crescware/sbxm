use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use crate::design::policy::{CharacterSet, StreamPolicy};
use crate::design::{RenderingPolicy, Ui};
use crate::i18n::Locale;
use crate::msg;
use crate::testing::outcome::{Checked, Required};
use crate::testing::scripted_clock::ScriptedClock;

use super::super::width::display_width;
use super::Trace;

fn policy(width: Option<usize>) -> StreamPolicy {
    StreamPolicy {
        color: false,
        characters: CharacterSet::Unicode,
        width,
    }
}

fn ms(value: u64) -> Duration {
    Duration::from_millis(value)
}

/// `act`が書いたtraceを、行ごとに返す。
fn lines(
    locale: Locale,
    policy: StreamPolicy,
    clock: &ScriptedClock,
    act: impl FnOnce(&mut Trace<'_>),
) -> Checked<Vec<String>> {
    let mut buffer: Vec<u8> = Vec::new();
    {
        let mut trace = Trace::new(locale, policy, &mut buffer, clock);
        act(&mut trace);
    }
    let text = String::from_utf8(buffer).required_because("UTF-8")?;
    Ok(text.lines().map(str::to_string).collect())
}

#[test]
fn a_captured_command_shows_its_status_and_sizes_with_the_time_at_the_right_edge() -> Checked {
    let clock = ScriptedClock::default();
    let lines = lines(Locale::En, policy(Some(72)), &clock, |trace| {
        let started = trace.run("sbx ls --json");
        clock.advance(Duration::from_micros(1_500));
        trace.done("sbx ls --json", started, "exit status: 0", 3, 0);
    })?;

    assert_eq!(lines[0], "run    sbx ls --json");
    let head = "done   sbx ls --json: exit status: 0, stdout 3B, stderr 0B";
    assert_eq!(
        lines[1],
        format!(
            "{head}{}1.500ms",
            " ".repeat(72 - head.len() - "1.500ms".len())
        )
    );
    assert_eq!(lines.len(), 2);
    Ok(())
}

#[test]
fn a_body_too_wide_for_the_line_keeps_one_space_before_the_time() -> Checked {
    let clock = ScriptedClock::default();
    let lines = lines(Locale::En, policy(Some(20)), &clock, |trace| {
        let started = trace.run("docker build --tag sbxm/example .");
        clock.advance(ms(1_204));
        trace.done_on_terminal(
            "docker build --tag sbxm/example .",
            started,
            "exit status: 0",
        );
    })?;

    assert_eq!(
        lines[1],
        "done   docker build --tag sbxm/example .: exit status: 0 1204.000ms"
    );
    Ok(())
}

#[test]
fn without_a_terminal_width_the_time_is_aligned_to_one_hundred_columns() -> Checked {
    let clock = ScriptedClock::default();
    let lines = lines(Locale::En, policy(None), &clock, |trace| {
        let started = trace.run("git --version");
        clock.advance(ms(2));
        trace.done("git --version", started, "exit status: 0", 24, 0);
    })?;

    assert_eq!(lines[1].len(), 100, "{:?}", lines[1]);
    assert!(lines[1].ends_with(" 2.000ms"), "{:?}", lines[1]);
    Ok(())
}

#[test]
fn the_width_counts_displayed_columns_rather_than_bytes() -> Checked {
    let clock = ScriptedClock::default();
    // 日本語の読点は3byteで2桁。byteで数えると右端が手前へずれる。
    let lines = lines(Locale::Ja, policy(Some(80)), &clock, |trace| {
        let started = trace.run("sbx ls --json");
        clock.advance(ms(1));
        trace.done("sbx ls --json", started, "exit status: 0", 3, 0);
    })?;

    assert!(
        lines[1].contains("、stdout 3B、stderr 0B"),
        "{:?}",
        lines[1]
    );
    assert_eq!(display_width(&lines[1]), 80);
    Ok(())
}

#[test]
fn a_step_is_timed_when_the_next_one_begins_and_the_last_when_the_trace_finishes() -> Checked {
    let clock = ScriptedClock::default();
    let lines = lines(Locale::En, policy(Some(60)), &clock, |trace| {
        trace.step("progress-inspect-docker");
        clock.advance(ms(2));
        trace.step("progress-inspect-sandbox");
        clock.advance(ms(3));
        trace.finish();
    })?;

    assert!(
        lines[0].starts_with("step   progress-inspect-docker "),
        "{lines:?}"
    );
    assert!(lines[0].ends_with(" 2.000ms"), "{lines:?}");
    assert!(
        lines[1].starts_with("step   progress-inspect-sandbox "),
        "{lines:?}"
    );
    assert!(lines[1].ends_with(" 3.000ms"), "{lines:?}");
    assert!(lines[2].starts_with("total  "), "{lines:?}");
    Ok(())
}

#[test]
fn the_total_splits_the_wall_time_between_external_commands_and_sbxm() -> Checked {
    let clock = ScriptedClock::default();
    let lines = lines(Locale::En, policy(Some(90)), &clock, |trace| {
        clock.advance(ms(4));
        let started = trace.run("sbx version");
        clock.advance(ms(10));
        trace.done("sbx version", started, "exit status: 0", 60, 0);
        let started = trace.run("sbx ls --json");
        clock.advance(ms(5));
        trace.done("sbx ls --json", started, "exit status: 0", 3, 0);
        clock.advance(ms(3));
        trace.finish();
    })?;

    let total = lines.last().required_because("a total line")?;
    assert!(
        total.starts_with("total  2 external commands 15.000ms, sbxm 7.000ms "),
        "{total:?}"
    );
    assert!(total.ends_with(" 22.000ms"), "{total:?}");
    assert_eq!(total.len(), 90);
    Ok(())
}

#[test]
fn a_single_external_command_is_counted_in_the_singular() -> Checked {
    let clock = ScriptedClock::default();
    let lines = lines(Locale::En, policy(Some(90)), &clock, |trace| {
        let started = trace.run("sbx version");
        clock.advance(ms(10));
        trace.done("sbx version", started, "exit status: 0", 60, 0);
        trace.finish();
    })?;

    assert!(
        lines[2].starts_with("total  1 external command 10.000ms, sbxm 0.000ms "),
        "{lines:?}"
    );
    Ok(())
}

#[test]
fn a_trace_that_recorded_nothing_leaves_no_total() -> Checked {
    let clock = ScriptedClock::default();
    let lines = lines(Locale::En, policy(Some(90)), &clock, |trace| {
        clock.advance(ms(10));
        trace.finish();
    })?;

    assert!(lines.is_empty(), "{lines:?}");
    Ok(())
}

#[test]
fn inside_lines_name_each_command_and_leave_the_time_blank_when_it_was_not_measured() -> Checked {
    let clock = ScriptedClock::default();
    let lines = lines(Locale::En, policy(Some(70)), &clock, |trace| {
        trace.inside(
            0,
            "printenv SSH_AUTH_SOCK",
            "exit status: 1",
            Some(Duration::from_nanos(3_102_000)),
        );
        trace.inside(1, "ssh-add -L", "exit status: 2", None);
    })?;

    assert!(
        lines[0].starts_with("inside 0 printenv SSH_AUTH_SOCK: exit status: 1 "),
        "{lines:?}"
    );
    assert!(lines[0].ends_with(" 3.102ms"), "{lines:?}");
    assert_eq!(lines[0].len(), 70);
    assert_eq!(lines[1], "inside 1 ssh-add -L: exit status: 2");
    Ok(())
}

#[test]
fn a_command_that_returned_no_result_is_traced_with_what_stopped_it() -> Checked {
    let clock = ScriptedClock::default();
    let lines = lines(Locale::En, policy(Some(70)), &clock, |trace| {
        let started = trace.run("sbx ls --json");
        clock.advance(ms(10_000));
        trace.failed("sbx ls --json", started, "external-command-timeout");
        trace.finish();
    })?;

    assert!(
        lines[1].starts_with("failed sbx ls --json: external-command-timeout "),
        "{lines:?}"
    );
    assert!(lines[1].ends_with(" 10000.000ms"), "{lines:?}");
    // 結果を持たずに終わった起動も、外部commandの中にいた時間に数える。
    assert!(
        lines[2].starts_with("total  1 external command 10000.000ms, sbxm 0.000ms "),
        "{lines:?}"
    );
    Ok(())
}

#[test]
fn colored_lines_are_dimmed_as_a_whole() -> Checked {
    let clock = ScriptedClock::default();
    let lines = lines(Locale::En, StreamPolicy::colored(), &clock, |trace| {
        trace.run("sbx version");
    })?;

    assert_eq!(lines[0], "\u{1b}[2mrun    sbx version\u{1b}[0m");
    Ok(())
}

#[test]
fn the_ui_reports_each_progress_step_to_the_attached_trace() -> Checked {
    let clock = ScriptedClock::default();
    let mut stdout: Vec<u8> = Vec::new();
    let mut stderr: Vec<u8> = Vec::new();
    let mut traced: Vec<u8> = Vec::new();
    {
        let trace = Rc::new(RefCell::new(Trace::new(
            Locale::En,
            policy(Some(60)),
            &mut traced,
            &clock,
        )));
        let mut ui = Ui::capture(
            Locale::En,
            RenderingPolicy::plain(),
            &mut stdout,
            &mut stderr,
        );
        ui.attach_trace(Rc::clone(&trace));
        ui.progress(msg!("progress-inspect-docker"));
        clock.advance(ms(7));
        ui.progress(msg!("progress-inspect-sandbox"));
        drop(ui);
        trace.borrow_mut().finish();
    }
    let traced = String::from_utf8(traced).required_because("UTF-8")?;
    let stderr = String::from_utf8(stderr).required_because("UTF-8")?;

    let lines: Vec<&str> = traced.lines().collect();
    assert!(
        lines[0].starts_with("step   progress-inspect-docker "),
        "{traced:?}"
    );
    assert!(lines[0].ends_with(" 7.000ms"), "{traced:?}");
    assert!(
        lines[1].starts_with("step   progress-inspect-sandbox "),
        "{traced:?}"
    );
    assert!(
        lines[2].starts_with("total  0 external commands 0.000ms, sbxm 7.000ms "),
        "{traced:?}"
    );
    // 進捗行そのものは変わらない。
    assert_eq!(
        stderr,
        "→ Checking Docker connectivity. Waiting for Docker to respond.\n\
         → Checking the sandbox state. Waiting for Docker Sandboxes to respond.\n"
    );
    Ok(())
}
