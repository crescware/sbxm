use crate::boundary::host::protocol::RootDiskUsage;
use crate::design::{Fact, StreamPolicy, renderer::Renderer};
use crate::diagnostics::{Diagnostic, ErrorId};
use crate::i18n::{Catalog, Locale};
use crate::testing::outcome::{Checked, Required};

use super::*;

fn draw(document: &Document, locale: Locale, policy: StreamPolicy) -> Checked<String> {
    let mut output = Vec::new();
    Renderer::new(&mut output, policy).write(&Catalog::new(locale), document);
    String::from_utf8(output).required()
}

fn sample(free_kib: u64) -> RootDiskUsage {
    RootDiskUsage {
        free_kib,
        usable_kib: 19_401_296,
        capacity_percent: 75,
    }
}

#[test]
fn english_shows_available_space_and_a_decreasing_gauge_instead_of_usage() -> Checked {
    let document = disk_section(
        Document::new(),
        &DiskObservation::Observed(sample(4_898_320)),
    );
    assert_eq!(
        draw(&document, Locale::En, StreamPolicy::plain())?,
        concat!(
            "DISK  /\n",
            "Available  4.7 GiB (25% remaining)\n",
            "           [█████░░░░░░░░░░░░░░░]  Low space\n",
            "Total      18.5 GiB\n",
        )
    );
    Ok(())
}

#[test]
fn japanese_uses_the_agreed_labels_without_redundant_english() -> Checked {
    let document = disk_section(
        Document::new(),
        &DiskObservation::Observed(sample(4_898_320)),
    );
    assert_eq!(
        draw(&document, Locale::Ja, StreamPolicy::plain())?,
        concat!(
            "ディスク  /\n",
            "空き容量  4.7 GiB（残り25%）\n",
            "          [█████░░░░░░░░░░░░░░░]  残り少なめ\n",
            "全体容量  18.5 GiB\n",
        )
    );
    Ok(())
}

#[test]
fn sub_gib_capacity_keeps_the_percentage_and_no_space_label_in_both_locales() -> Checked {
    for (free, english, japanese, remaining) in [
        (96 * 1024, "No space available", "空きなし", "<1%"),
        (512 * 1024, "Very low space", "残りわずか", "2%"),
    ] {
        let document = disk_section(Document::new(), &DiskObservation::Observed(sample(free)));
        for (locale, status) in [(Locale::En, english), (Locale::Ja, japanese)] {
            let output = draw(&document, locale, StreamPolicy::plain())?;
            assert!(output.contains(" MiB"), "{output}");
            assert!(output.contains(remaining), "{output}");
            assert!(output.contains(status), "{output}");
            assert!(!output.contains("KiB"), "{output}");
        }
    }
    Ok(())
}

#[test]
fn ascii_output_keeps_the_gauge_and_warning_without_ansi() -> Checked {
    let document = disk_section(
        Document::new(),
        &DiskObservation::Observed(sample(4_898_320)),
    );
    let output = draw(&document, Locale::En, StreamPolicy::ascii())?;
    assert!(
        output.contains("[#####---------------]  Low space"),
        "{output}"
    );
    assert!(output.is_ascii(), "{output}");
    assert!(!output.contains('\u{1b}'));
    Ok(())
}

#[test]
fn the_gauge_has_twenty_cells_at_zero_partial_and_full_capacity() -> Checked {
    for (free, filled, empty, percentage) in [
        (0, 0, 20, "0% remaining"),
        (8_493_466, 9, 11, "43% remaining"),
        (19_401_296, 20, 0, "100% remaining"),
    ] {
        let document = disk_section(Document::new(), &DiskObservation::Observed(sample(free)));
        let output = draw(&document, Locale::En, StreamPolicy::ascii())?;
        assert!(
            output.contains(&format!("[{}{}]", "#".repeat(filled), "-".repeat(empty))),
            "{output}"
        );
        assert!(output.contains(percentage), "{output}");
        if free >= 8_493_466 {
            assert!(!output.contains("Low space"), "{output}");
            assert!(!output.contains("No space"), "{output}");
        }
    }
    Ok(())
}

#[test]
fn the_gauge_colors_follow_remaining_space_without_coloring_the_whole_line() -> Checked {
    for (free, color) in [(8_493_466, "32"), (4_898_320, "33"), (96 * 1024, "31")] {
        let document = disk_section(Document::new(), &DiskObservation::Observed(sample(free)));
        let output = draw(&document, Locale::En, StreamPolicy::colored())?;
        assert!(output.contains(&format!("\u{1b}[{color}m")), "{output:?}");
        assert!(output.contains("[\u{1b}"), "{output:?}");
        assert!(!output.contains("\u{1b}[48;"), "{output:?}");
        assert!(!output.contains("38;2;"), "{output:?}");
    }
    Ok(())
}

#[test]
fn failed_observations_have_a_reason_and_no_fake_zero_percent_gauge() -> Checked {
    for observation in [
        DiskObservation::NotObservedStopped,
        DiskObservation::NotObservedNotCreated,
        DiskObservation::NotObservedMismatch,
        DiskObservation::CommandMissing,
        DiskObservation::ParseFailed,
        DiskObservation::Observed(RootDiskUsage {
            free_kib: 0,
            usable_kib: 0,
            capacity_percent: 0,
        }),
    ] {
        let output = draw(
            &disk_section(Document::new(), &observation),
            Locale::En,
            StreamPolicy::plain(),
        )?;
        assert!(output.starts_with("DISK\n"), "{output}");
        assert!(!output.contains("remaining"), "{output}");
        assert!(!output.contains('['), "{output}");
        assert!(output.lines().count() >= 2, "{output}");
    }
    Ok(())
}

#[test]
fn failure_diagnostics_use_the_same_localized_meter_with_diagnostic_indentation() -> Checked {
    let meter = disk_meter(&sample(4_898_320)).required()?;
    let document = Document::new().diagnostic(
        Diagnostic::new(
            ErrorId::ExternalCommandFailed,
            msg!(
                "error-external-command-failed",
                exit_status = "exit status: 28"
            ),
        )
        .fact(Fact::Capacity {
            label: msg!("diagnostic-disk-label"),
            meter: Box::new(meter),
        }),
    );
    for (locale, disk, available, total, status) in [
        (Locale::En, "Disk:", "Available", "Total", "Low space"),
        (
            Locale::Ja,
            "ディスク:",
            "空き容量",
            "全体容量",
            "残り少なめ",
        ),
    ] {
        let output = draw(&document, locale, StreamPolicy::plain())?;
        assert!(
            output.contains(&format!("  {disk} /\n    {available}")),
            "{output}"
        );
        assert!(output.contains(&format!("    {total}")), "{output}");
        assert!(output.contains("25%"), "{output}");
        assert!(output.contains(status), "{output}");
    }
    Ok(())
}
