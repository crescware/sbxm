use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::error::ErrorId;

#[test]
fn versions_require_exactly_three_numeric_parts() {
    assert_eq!(
        CliVersion::parse("0.37.0"),
        Some(CliVersion {
            major: 0,
            minor: 37,
            patch: 0
        })
    );
    assert_eq!(CliVersion::parse("0.37"), None);
    assert_eq!(CliVersion::parse("0.37.0.1"), None);
    assert_eq!(CliVersion::parse("0.37.x"), None);
    assert_eq!(CliVersion::parse(""), None);
}

#[test]
fn versions_are_extracted_from_surrounding_text() {
    assert_eq!(
        CliVersion::extract_from_output("sbx version 0.37.2\n"),
        CliVersion::parse("0.37.2")
    );
    assert_eq!(
        CliVersion::extract_from_output("Docker Sandboxes CLI v1.2.3 (build abc)"),
        CliVersion::parse("1.2.3")
    );
    assert_eq!(CliVersion::extract_from_output("no version here"), None);
    assert_eq!(CliVersion::extract_from_output(""), None);
}

#[test]
fn versions_below_the_minimum_are_refused() -> Checked {
    let error = require_minimum_version(CliVersion::parse("0.36.9").required()?)
        .refused_because("an older version must be refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::SbxVersionBelowMinimum));
    Ok(())
}

#[test]
fn the_minimum_version_and_later_are_accepted() -> Checked {
    for observed in ["0.37.0", "0.37.5", "0.38.0", "1.0.0"] {
        assert!(
            require_minimum_version(CliVersion::parse(observed).required()?).is_ok(),
            "{observed} must be accepted"
        );
    }
    Ok(())
}
