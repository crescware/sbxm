use std::collections::BTreeMap;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::boundary::host::Executable;
use crate::boundary::host::protocol::CliVersion;
use crate::config::ConfigLocation;
use crate::testing::fs::temp_dir;
use crate::testing::global_status::{location_with_config, location_with_state_dir};
use crate::testing::host::reusable_executable;
use crate::testing::outcome::{Checked, Required};
use crate::testing::recorded_output::RecordedOutput;

use super::*;

fn version(text: &str) -> Checked<CliVersion> {
    CliVersion::parse(text).required_because(text)
}

/// `0600`のfileとして書く。
fn write_private(path: &Path, text: &str) -> Checked {
    std::fs::write(path, text).required()?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).required()?;
    Ok(())
}

fn records_of(pairs: &[(&str, &Executable, &str)]) -> Checked<BTreeMap<String, RecordedTool>> {
    let mut records = BTreeMap::new();
    for (program, executable, observed) in pairs {
        records.insert(
            (*program).to_string(),
            RecordedTool::new(program, executable.identity.clone(), version(observed)?),
        );
    }
    Ok(records)
}

/// 最後のtraceのmessage IDと、`reason`引数。
fn last_trace(progress: &RecordedOutput) -> Checked<(&'static str, String)> {
    let last = progress.traces.last().required_because("a trace line")?;
    let reason = last
        .args
        .iter()
        .find(|(key, _)| *key == "reason")
        .map(|(_, value)| value.clone())
        .unwrap_or_default();
    Ok((last.id, reason))
}

fn kind(loaded: &Loaded) -> &'static str {
    match loaded {
        Loaded::Missing => "missing",
        Loaded::Unusable => "unusable",
        Loaded::Corrupt => "corrupt",
        Loaded::Records(_) => "records",
    }
}

fn skipped(
    location: &ConfigLocation,
    program: &str,
    executable: Option<&Executable>,
) -> Checked<Option<CliVersion>> {
    let mut progress = RecordedOutput::new();
    Ok(recorded_version(
        location,
        program,
        executable,
        version("1.0.0")?,
        &mut progress,
    ))
}

/// 記録を使わなかった理由。
fn why_checked(
    location: &ConfigLocation,
    program: &str,
    executable: Option<&Executable>,
    minimum: &str,
) -> Checked<String> {
    let mut progress = RecordedOutput::new();
    let outcome = recorded_version(
        location,
        program,
        executable,
        version(minimum)?,
        &mut progress,
    );
    assert_eq!(outcome, None);
    let (id, reason) = last_trace(&progress)?;
    assert_eq!(id, "trace-host-tool-checked");
    Ok(reason)
}

/// 記録しなかった理由。
fn why_unsaved(
    location: &ConfigLocation,
    program: &str,
    executable: Option<&Executable>,
) -> Checked<String> {
    let mut progress = RecordedOutput::new();
    record(
        location,
        program,
        executable,
        version("1.0.0")?,
        &mut progress,
    );
    let (id, reason) = last_trace(&progress)?;
    assert_eq!(id, "trace-host-tool-unsaved");
    Ok(reason)
}

#[test]
fn the_digest_covers_every_field_and_the_program_name() -> Checked {
    let sbx = reusable_executable("sbx");
    let observed = version("0.48.0")?;
    let digest_of = |program: &str, executable: &Executable, observed: CliVersion| {
        digest(program, &executable.identity, observed)
    };

    let base = digest_of("sbx", &sbx, observed);
    assert_eq!(base.len(), 64);
    assert_eq!(
        base,
        digest_of("sbx", &sbx, observed),
        "the digest is deterministic"
    );
    assert_ne!(
        base,
        digest_of("git", &sbx, observed),
        "the program name is covered"
    );
    assert_ne!(base, digest_of("sbx", &sbx, version("0.49.0")?));

    let mut changed = sbx.clone();
    changed.identity.size += 1;
    assert_ne!(base, digest_of("sbx", &changed, observed));
    let mut changed = sbx.clone();
    changed.identity.ctime_nanos += 1;
    assert_ne!(base, digest_of("sbx", &changed, observed));
    let mut changed = sbx;
    changed.identity.path = PathBuf::from("/usr/local/bin/sbx");
    assert_ne!(base, digest_of("sbx", &changed, observed));
    Ok(())
}

#[test]
fn a_record_survives_a_round_trip_through_yaml() -> Checked {
    let records = records_of(&[
        ("sbx", &reusable_executable("sbx"), "0.48.0"),
        ("git", &reusable_executable("git"), "2.47.0"),
    ])?;
    let text = render(&records).required()?;
    assert!(text.contains("version: 1"), "{text}");
    assert!(text.contains("sbx:"), "{text}");
    assert!(
        text.contains("/opt/homebrew/Cellar/git/1.0.0/bin/git"),
        "{text}"
    );
    assert_eq!(
        parse(&text).required_because("the rendered text is read back")?,
        records
    );
    Ok(())
}

#[test]
fn records_that_cannot_be_trusted_are_not_read() -> Checked {
    let records = records_of(&[("sbx", &reusable_executable("sbx"), "0.48.0")])?;
    let valid = render(&records).required()?;
    assert!(parse(&valid).is_some());

    for (name, text) in [
        ("an empty document", String::new()),
        ("a list", "- 1\n".to_string()),
        ("no version", valid.replace("version: 1\n", "")),
        (
            "another version",
            valid.replace("version: 1\n", "version: 2\n"),
        ),
        (
            "an unknown top-level key",
            format!("{valid}checked_at: 1\n"),
        ),
        (
            "an unknown tool key",
            valid.replace("    size:", "    extra: 1\n    size:"),
        ),
        (
            "a missing field",
            valid
                .lines()
                .filter(|line| !line.contains("inode"))
                .flat_map(|line| [line, "\n"])
                .collect(),
        ),
        ("an unreadable version", valid.replace("0.48.0", "latest")),
        (
            "a non-numeric size",
            valid.replace("size: 4096", "size: big"),
        ),
    ] {
        assert!(parse(&text).is_none(), "{name} must not be read:\n{text}");
    }
    Ok(())
}

#[test]
fn the_record_file_must_be_a_private_regular_file_owned_by_the_user() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("host-tools.yaml");
    assert_eq!(kind(&load(&path)), "missing");

    let records = records_of(&[("sbx", &reusable_executable("sbx"), "0.48.0")])?;
    write_private(&path, &render(&records).required()?)?;
    let Loaded::Records(loaded) = load(&path) else {
        return Err(crate::testing::outcome::Unmet::new(
            "a private record is read".to_string(),
        ));
    };
    assert_eq!(loaded, records);

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).required()?;
    assert_eq!(
        kind(&load(&path)),
        "unusable",
        "group or others can read it"
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).required()?;

    write_private(&path, "version: [\n")?;
    assert_eq!(kind(&load(&path)), "corrupt");

    std::fs::remove_file(&path).required()?;
    std::fs::create_dir(&path).required()?;
    assert_eq!(
        kind(&load(&path)),
        "unusable",
        "a directory is not a record"
    );
    std::fs::remove_dir(&path).required()?;

    let target = dir.path().join("elsewhere.yaml");
    write_private(&target, &render(&records).required()?)?;
    std::os::unix::fs::symlink(&target, &path).required()?;
    assert_eq!(kind(&load(&path)), "unusable", "a symlink is not followed");
    Ok(())
}

#[test]
fn a_matching_record_skips_the_check_and_says_so_in_the_trace() -> Checked {
    let (_dir, location) = location_with_state_dir()?;
    let sbx = reusable_executable("sbx");
    let mut progress = RecordedOutput::new();
    record(
        &location,
        "sbx",
        Some(&sbx),
        version("0.48.0")?,
        &mut progress,
    );
    assert!(progress.traces.is_empty(), "recording succeeded silently");

    let mut progress = RecordedOutput::new();
    let outcome = recorded_version(
        &location,
        "sbx",
        Some(&sbx),
        version("0.48.0")?,
        &mut progress,
    );
    assert_eq!(outcome, Some(version("0.48.0")?));
    let trace = progress.traces.last().required()?;
    assert_eq!(trace.id, "trace-host-tool-skipped");
    assert_eq!(
        trace.args,
        [
            ("program", "sbx".to_string()),
            ("observed", "0.48.0".to_string()),
            ("path", "/opt/homebrew/Cellar/sbx/1.0.0/bin/sbx".to_string()),
        ]
    );

    // 記録されたversionが新しい最低versionも満たすなら、sbxmを更新しても省略できる。
    assert_eq!(skipped(&location, "sbx", Some(&sbx))?, None);
    assert_eq!(
        why_checked(&location, "sbx", Some(&sbx), "0.49.0")?,
        "recorded-version-below-minimum"
    );
    assert_eq!(
        recorded_version(
            &location,
            "sbx",
            Some(&sbx),
            version("0.47.0")?,
            &mut RecordedOutput::new()
        ),
        Some(version("0.48.0")?)
    );
    Ok(())
}

#[test]
fn every_reason_the_record_is_not_used_is_traced() -> Checked {
    let (_dir, location) = location_with_state_dir()?;
    let sbx = reusable_executable("sbx");
    let minimum = "0.48.0";

    assert_eq!(
        why_checked(&location, "sbx", None, minimum)?,
        "executable-unobserved"
    );

    let mut script = sbx.clone();
    script.script = true;
    assert_eq!(
        why_checked(&location, "sbx", Some(&script), minimum)?,
        "script"
    );
    let mut shim = sbx.clone();
    shim.identity.path = PathBuf::from("/opt/homebrew/Cellar/mise/1.0.0/bin/mise");
    assert_eq!(
        why_checked(&location, "sbx", Some(&shim), minimum)?,
        "name-mismatch"
    );
    let mut apple = reusable_executable("git");
    apple.identity.path = PathBuf::from("/usr/bin/git");
    assert_eq!(
        why_checked(&location, "git", Some(&apple), minimum)?,
        "apple-shim"
    );

    assert_eq!(
        why_checked(&location, "sbx", Some(&sbx), minimum)?,
        "record-missing",
        "no file yet"
    );
    let path = location.host_tools_file();
    let git_only = records_of(&[("git", &reusable_executable("git"), "2.47.0")])?;
    write_private(&path, &render(&git_only).required()?)?;
    assert_eq!(
        why_checked(&location, "sbx", Some(&sbx), minimum)?,
        "record-missing",
        "no sbx entry"
    );

    write_private(&path, "version: [\n")?;
    assert_eq!(
        why_checked(&location, "sbx", Some(&sbx), minimum)?,
        "record-corrupt"
    );

    let sbx_records = records_of(&[("sbx", &sbx, "0.48.0")])?;
    write_private(&path, &render(&sbx_records).required()?)?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).required()?;
    assert_eq!(
        why_checked(&location, "sbx", Some(&sbx), minimum)?,
        "record-unusable"
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).required()?;

    let mut upgraded = sbx.clone();
    upgraded.identity.path = PathBuf::from("/opt/homebrew/Cellar/sbx/1.1.0/bin/sbx");
    assert_eq!(
        why_checked(&location, "sbx", Some(&upgraded), minimum)?,
        "executable-changed"
    );
    let mut rewritten = sbx.clone();
    rewritten.identity.mtime_secs += 1;
    assert_eq!(
        why_checked(&location, "sbx", Some(&rewritten), minimum)?,
        "executable-changed"
    );

    // observedだけを書き換えた記録は、digestが合わない。
    let mut tampered = sbx_records;
    let entry = tampered.get_mut("sbx").required()?;
    entry.observed = version("9.9.9")?;
    write_private(&path, &render(&tampered).required()?)?;
    assert_eq!(
        why_checked(&location, "sbx", Some(&sbx), minimum)?,
        "digest-mismatch"
    );
    Ok(())
}

#[test]
fn recording_merges_with_other_tools_and_keeps_the_file_private() -> Checked {
    let (_dir, location) = location_with_state_dir()?;
    let sbx = reusable_executable("sbx");
    let git = reusable_executable("git");
    let mut progress = RecordedOutput::new();
    record(
        &location,
        "sbx",
        Some(&sbx),
        version("0.48.0")?,
        &mut progress,
    );
    record(
        &location,
        "git",
        Some(&git),
        version("2.47.0")?,
        &mut progress,
    );
    assert!(progress.traces.is_empty());

    let path = location.host_tools_file();
    let mode = std::fs::metadata(&path).required()?.permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
    let Loaded::Records(records) = load(&path) else {
        return Err(crate::testing::outcome::Unmet::new(
            "the record is read back".to_string(),
        ));
    };
    assert_eq!(
        records,
        records_of(&[("git", &git, "2.47.0"), ("sbx", &sbx, "0.48.0")])?
    );

    // 同じ道具の記録は置き換わる。別の道具の記録は残る。
    let mut upgraded = sbx.clone();
    upgraded.identity.path = PathBuf::from("/opt/homebrew/Cellar/sbx/1.1.0/bin/sbx");
    record(
        &location,
        "sbx",
        Some(&upgraded),
        version("0.49.0")?,
        &mut progress,
    );
    assert!(progress.traces.is_empty());
    assert_eq!(
        why_checked(&location, "sbx", Some(&sbx), "0.48.0")?,
        "executable-changed"
    );
    assert_eq!(
        skipped(&location, "git", Some(&git))?,
        Some(version("2.47.0")?)
    );
    assert_eq!(
        skipped(&location, "sbx", Some(&upgraded))?,
        None,
        "1.0.0 is above 0.49.0"
    );
    assert_eq!(
        recorded_version(
            &location,
            "sbx",
            Some(&upgraded),
            version("0.49.0")?,
            &mut RecordedOutput::new()
        ),
        Some(version("0.49.0")?)
    );

    // 解釈できない記録は、確認が通れば置き換わる。
    write_private(&path, "version: [\n")?;
    record(
        &location,
        "sbx",
        Some(&sbx),
        version("0.48.0")?,
        &mut progress,
    );
    assert!(progress.traces.is_empty());
    assert_eq!(
        recorded_version(
            &location,
            "sbx",
            Some(&sbx),
            version("0.48.0")?,
            &mut RecordedOutput::new()
        ),
        Some(version("0.48.0")?)
    );
    Ok(())
}

#[test]
fn every_reason_the_record_is_not_saved_is_traced() -> Checked {
    let sbx = reusable_executable("sbx");

    let (_home, without_state_dir) = location_with_config(None)?;
    assert_eq!(
        why_unsaved(&without_state_dir, "sbx", Some(&sbx))?,
        "state-directory-missing"
    );
    assert!(
        !without_state_dir.dir().exists(),
        "the state directory is not created for a record"
    );

    let (_dir, location) = location_with_state_dir()?;
    assert_eq!(
        why_unsaved(&location, "sbx", None)?,
        "executable-unobserved"
    );
    let mut script = sbx.clone();
    script.script = true;
    assert_eq!(why_unsaved(&location, "sbx", Some(&script))?, "script");
    assert!(!location.host_tools_file().exists());

    // 要件を満たさないfileは置き換えない。symlinkはsymlinkのまま残る。
    let path = location.host_tools_file();
    let target = location.dir().join("elsewhere.yaml");
    write_private(&target, "")?;
    std::os::unix::fs::symlink(&target, &path).required()?;
    assert_eq!(
        why_unsaved(&location, "sbx", Some(&sbx))?,
        "record-unusable"
    );
    assert!(
        std::fs::symlink_metadata(&path)
            .required()?
            .file_type()
            .is_symlink()
    );
    assert_eq!(std::fs::read_to_string(&target).required()?, "");
    std::fs::remove_file(&path).required()?;

    if !rustix::process::geteuid().is_root() {
        // 書けないdirectoryでは記録できないが、確認は通っているのでerrorにはしない。
        std::fs::set_permissions(location.dir(), std::fs::Permissions::from_mode(0o500))
            .required()?;
        let reason = why_unsaved(&location, "sbx", Some(&sbx));
        std::fs::set_permissions(location.dir(), std::fs::Permissions::from_mode(0o700))
            .required()?;
        assert_eq!(reason?, "write-failed");
    }
    Ok(())
}

#[test]
fn reasons_have_stable_untranslated_names() {
    let reasons = [
        (Unused::ExecutableUnobserved, "executable-unobserved"),
        (
            Unused::NotReusable(NotReusable::NameMismatch),
            "name-mismatch",
        ),
        (Unused::NotReusable(NotReusable::Script), "script"),
        (Unused::NotReusable(NotReusable::AppleShim), "apple-shim"),
        (Unused::RecordMissing, "record-missing"),
        (Unused::RecordUnusable, "record-unusable"),
        (Unused::RecordCorrupt, "record-corrupt"),
        (Unused::ExecutableChanged, "executable-changed"),
        (Unused::DigestMismatch, "digest-mismatch"),
        (
            Unused::VersionBelowMinimum,
            "recorded-version-below-minimum",
        ),
    ];
    for (reason, name) in reasons {
        assert_eq!(reason.as_str(), name);
    }
    let unsaved = [
        (Unsaved::ExecutableUnobserved, "executable-unobserved"),
        (Unsaved::NotReusable(NotReusable::Script), "script"),
        (Unsaved::StateDirectoryMissing, "state-directory-missing"),
        (Unsaved::RecordUnusable, "record-unusable"),
        (Unsaved::WriteFailed, "write-failed"),
    ];
    for (reason, name) in unsaved {
        assert_eq!(reason.as_str(), name);
    }
}

#[test]
fn a_record_that_cannot_be_opened_or_decoded_is_corrupt_rather_than_trusted() -> Checked {
    let dir = temp_dir()?;
    let path = dir.path().join("host-tools.yaml");
    std::fs::write(&path, [0xff, 0xfe, 0xfd]).required()?;
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).required()?;
    assert_eq!(kind(&load(&path)), "corrupt", "not UTF-8");

    if !rustix::process::geteuid().is_root() {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o000)).required()?;
        let loaded = kind(&load(&path));
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).required()?;
        assert_eq!(loaded, "corrupt", "a file that cannot be opened");
    }
    Ok(())
}
