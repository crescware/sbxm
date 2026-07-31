use super::*;

#[test]
fn exit_codes_are_the_published_contract() {
    assert_eq!(ExitCode::Success.as_i32(), 0);
    assert_eq!(ExitCode::Failure.as_i32(), 1);
    assert_eq!(ExitCode::Canceled.as_i32(), 130);
}

#[test]
fn cancellation_maps_to_130_and_everything_else_to_1() {
    assert_eq!(Error::Canceled.exit_code(), ExitCode::Canceled);
    assert_eq!(
        Error::new(ErrorId::ConfigUnreadable, msg!("config-unreadable")).exit_code(),
        ExitCode::Failure
    );
}

#[test]
fn error_ids_are_stable_kebab_case_ascii() {
    for id in ErrorId::ALL {
        let text = id.as_str();
        assert!(!text.is_empty(), "{id:?} has an empty error ID");
        assert!(
            text.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-'),
            "{text} is not stable lowercase kebab-case"
        );
        assert!(
            !text.starts_with('-') && !text.ends_with('-'),
            "{text} must not start or end with a hyphen"
        );
    }
}

#[test]
fn error_ids_are_unique() {
    let mut seen: Vec<&'static str> = ErrorId::ALL.iter().map(|id| id.as_str()).collect();
    let total = seen.len();
    seen.sort_unstable();
    seen.dedup();
    assert_eq!(seen.len(), total, "duplicate error ID strings detected");
}

#[test]
fn msg_macro_collects_named_arguments() {
    let message = msg!("config-invalid-syntax", path = "/tmp/x", detail = 42);
    assert_eq!(message.id, "config-invalid-syntax");
    assert_eq!(
        message.args,
        vec![("path", "/tmp/x".to_string()), ("detail", "42".to_string())]
    );
}

#[test]
fn external_failure_keeps_raw_stderr() {
    let failure = ExternalFailure {
        program: "sbx".into(),
        safe_args: vec!["ls".into()],
        working_dir: None,
        exit_status: "exit status: 2".into(),
        stderr: b"boom\n".to_vec(),
        stderr_lossy: false,
    };
    assert_eq!(failure.stderr_text(), "boom\n");
}

/// 未知versionの診断を組み立てる文書定義。
fn document() -> DocumentVersion {
    DocumentVersion {
        supported: 1,
        unknown: ErrorId::ConfigUnknownVersion,
        unknown_message: "error-config-unknown-version",
    }
}

/// version欄が無い場合に呼ばれたことを示す診断。
fn missing() -> Error {
    Error::new(
        ErrorId::ConfigMissingField,
        msg!("error-config-missing-field"),
    )
}

#[test]
fn an_unknown_version_names_the_path_the_value_found_and_the_version_supported() {
    let error = document()
        .require(Some(99), "/tmp/x", missing)
        .expect_err("this build reads version 1 only");
    let diagnostic = &error.diagnostics()[0];
    assert_eq!(diagnostic.id, ErrorId::ConfigUnknownVersion);
    assert_eq!(diagnostic.description.id, "error-config-unknown-version");
    assert_eq!(
        diagnostic.description.args,
        vec![
            ("path", "/tmp/x".to_string()),
            ("version", "99".to_string()),
            ("supported", "1".to_string())
        ]
    );
}

#[test]
fn the_supported_version_is_accepted_and_a_missing_version_is_left_to_the_caller() {
    assert!(document().require(Some(1), "/tmp/x", missing).is_ok());
    let error = document()
        .require(None, "/tmp/x", missing)
        .expect_err("a document without a version field is refused");
    assert_eq!(error.first_id(), Some(ErrorId::ConfigMissingField));
}
