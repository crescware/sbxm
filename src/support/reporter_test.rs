use super::*;
use crate::error::{ErrorId, ExternalFailure};
use crate::i18n::Locale;
use crate::msg;
fn rows() -> Vec<Row> {
    vec![
        Row {
            item: "status-item-config",
            status: StatusValue::Ready,
        },
        Row {
            item: "status-item-daemon",
            status: StatusValue::Error,
        },
    ]
}
#[test]
fn the_english_table_keeps_the_published_column_names() {
    let catalog = Catalog::new(Locale::En);
    let reporter = Reporter::new(&catalog);
    let table = reporter.render_status_table(
        "status-global-section",
        "status-column-item",
        "status-column-status",
        &rows(),
    );
    let lines: Vec<&str> = table.lines().collect();
    assert_eq!(lines[0], "GLOBAL");
    assert!(lines[1].starts_with("ITEM"), "{}", lines[1]);
    assert!(lines[1].ends_with("STATUS"), "{}", lines[1]);
    assert!(lines[2].starts_with("Config"), "{}", lines[2]);
    assert!(lines[2].ends_with("ready"), "{}", lines[2]);
    assert!(lines[3].ends_with("error"), "{}", lines[3]);
}

#[test]
fn columns_line_up_in_both_languages() {
    for locale in Locale::ALL {
        let catalog = Catalog::new(locale);
        let reporter = Reporter::new(&catalog);
        let table = reporter.render_status_table(
            "status-global-section",
            "status-column-item",
            "status-column-status",
            &rows(),
        );
        let lines: Vec<&str> = table.lines().collect();
        let status_header = catalog.text("status-column-status").unwrap();

        let mut offsets = vec![display_width(
            lines[1]
                .strip_suffix(&status_header)
                .expect("the header line ends with the status column name"),
        )];
        for (row, line) in rows().iter().zip(lines.iter().skip(2)) {
            offsets.push(display_width(
                line.strip_suffix(row.status.as_str())
                    .expect("each row ends with its status value"),
            ));
        }
        assert!(
            offsets.windows(2).all(|pair| pair[0] == pair[1]),
            "{locale}: the status column must start at one fixed offset: {offsets:?}"
        );
    }
}

#[test]
fn status_values_are_never_translated() {
    for locale in Locale::ALL {
        let catalog = Catalog::new(locale);
        let reporter = Reporter::new(&catalog);
        let table = reporter.render_status_table(
            "status-global-section",
            "status-column-item",
            "status-column-status",
            &rows(),
        );
        assert!(table.contains("ready"), "{locale}: {table}");
        assert!(table.contains("error"), "{locale}: {table}");
    }
}

#[test]
fn the_legend_lists_only_the_values_that_actually_appeared() {
    for locale in Locale::ALL.into_iter().filter(|locale| !locale.is_source()) {
        let catalog = Catalog::new(locale);
        let reporter = Reporter::new(&catalog);
        let legend = reporter
            .render_legend(&rows())
            .unwrap_or_else(|| panic!("{locale} is not the source locale, so it adds a legend"));
        assert!(legend.contains("ready:"), "{locale}: {legend}");
        assert!(legend.contains("error:"), "{locale}: {legend}");
        assert!(
            !legend.contains("stopped:"),
            "{locale}: values that did not appear must be left out: {legend}"
        );
    }
}

#[test]
fn the_source_locale_has_no_legend() {
    // 状態値は正本localeの語であるため、正本localeでは注釈を出さない。
    let catalog = Catalog::new(Locale::SOURCE);
    let reporter = Reporter::new(&catalog);
    assert!(reporter.render_legend(&rows()).is_none());
}

#[test]
fn errors_show_the_stable_id_the_description_and_the_remediation() {
    let catalog = Catalog::new(Locale::En);
    let reporter = Reporter::new(&catalog);
    let error = Error::single(
        Diagnostic::new(
            ErrorId::ConfigMissing,
            msg!(
                "error-config-missing",
                path = "/home/example/.sbxm/config.toml"
            ),
        )
        .remediation(msg!("remediation-run-init")),
    );

    let mut buffer = Vec::new();
    reporter.print_error(&error, &mut buffer);
    let text = String::from_utf8(buffer).unwrap();

    assert!(text.starts_with("error: config-missing\n"), "{text}");
    assert!(text.contains("/home/example/.sbxm/config.toml"), "{text}");
    assert!(text.contains("sbxm init"), "{text}");
}

#[test]
fn external_stderr_is_shown_verbatim_in_its_own_block() {
    let catalog = Catalog::new(Locale::Ja);
    let reporter = Reporter::new(&catalog);
    let error = Error::single(
        Diagnostic::new(
            ErrorId::ExternalCommandFailed,
            msg!(
                "error-external-command-failed",
                program = "sbx",
                exit_status = "exit status: 2"
            ),
        )
        .external(ExternalFailure {
            program: "sbx".into(),
            safe_args: vec!["ls".into()],
            working_dir: None,
            exit_status: "exit status: 2".into(),
            stderr: b"Error: daemon is not running".to_vec(),
            stderr_lossy: false,
        }),
    );

    let mut buffer = Vec::new();
    reporter.print_error(&error, &mut buffer);
    let text = String::from_utf8(buffer).unwrap();

    assert!(text.contains("error: external-command-failed"), "{text}");
    assert!(
        text.contains("Error: daemon is not running"),
        "the external message must be preserved verbatim: {text}"
    );
    assert!(text.ends_with('\n'), "{text:?}");
}

#[test]
fn a_failed_command_is_shown_with_the_invocation_that_produced_it() {
    let catalog = Catalog::new(Locale::En);
    let reporter = Reporter::new(&catalog);
    let error = Error::single(
        Diagnostic::new(
            ErrorId::ExternalCommandFailed,
            msg!(
                "error-external-command-failed",
                program = "git",
                exit_status = "exit status: 128"
            ),
        )
        .external(ExternalFailure {
            program: "git".into(),
            safe_args: vec!["clone".into(), "--bare".into()],
            working_dir: Some(std::path::PathBuf::from("/Users/example/Projects")),
            exit_status: "exit status: 128".into(),
            stderr: Vec::new(),
            stderr_lossy: false,
        }),
    );

    let mut buffer = Vec::new();
    reporter.print_error(&error, &mut buffer);
    let text = String::from_utf8(buffer).unwrap();

    assert!(text.contains("git clone --bare"), "{text}");
    assert!(text.contains("/Users/example/Projects"), "{text}");
}

#[test]
fn a_lossy_external_stream_is_reported_as_such() {
    let catalog = Catalog::new(Locale::En);
    let reporter = Reporter::new(&catalog);
    let error = Error::single(
        Diagnostic::new(
            ErrorId::ExternalCommandFailed,
            msg!(
                "error-external-command-failed",
                program = "sbx",
                exit_status = "exit status: 1"
            ),
        )
        .external(ExternalFailure {
            program: "sbx".into(),
            safe_args: Vec::new(),
            working_dir: None,
            exit_status: "exit status: 1".into(),
            stderr: vec![0xff, b'a'],
            stderr_lossy: true,
        }),
    );

    let mut buffer = Vec::new();
    reporter.print_error(&error, &mut buffer);
    let text = String::from_utf8_lossy(&buffer);
    assert!(text.contains("not valid UTF-8"), "{text}");
}

#[test]
fn a_table_with_no_rows_still_shows_its_header() {
    let catalog = Catalog::new(Locale::En);
    let table = Reporter::new(&catalog)
        .render_value_table(&["column-project", "column-sandbox", "column-state"], &[]);
    assert_eq!(table.lines().count(), 1);
    let header = table.lines().next().expect("the header line");
    assert!(header.contains("PROJECT"), "{header}");
    assert!(header.contains("SANDBOX"), "{header}");
    assert!(header.contains("STATE"), "{header}");
}

#[test]
fn a_canceled_run_prints_nothing() {
    let catalog = Catalog::new(Locale::En);
    let mut buffer: Vec<u8> = Vec::new();
    Reporter::new(&catalog).print_error(&Error::Canceled, &mut buffer);
    assert!(buffer.is_empty());
}

#[test]
fn every_diagnostic_of_a_multi_error_run_is_shown() {
    let catalog = Catalog::new(Locale::En);
    let reporter = Reporter::new(&catalog);
    let error = Error::many(vec![
        Diagnostic::new(
            ErrorId::ConfigMissing,
            msg!("error-config-missing", path = "/a"),
        ),
        Diagnostic::new(
            ErrorId::HostCommandMissing,
            msg!("error-host-command-missing", command = "sbx"),
        ),
    ]);

    let mut buffer = Vec::new();
    reporter.print_error(&error, &mut buffer);
    let text = String::from_utf8(buffer).unwrap();
    assert!(text.contains("error: config-missing"), "{text}");
    assert!(text.contains("error: host-command-missing"), "{text}");
}
