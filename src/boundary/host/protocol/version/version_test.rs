use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::diagnostics::ErrorId;

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
    assert_eq!(
        CliVersion::extract_from_output(
            "sbx version v0.48.0 5caf923e0587d376848f6f388eca3e0f5ca00e2d\n"
        ),
        CliVersion::parse("0.48.0")
    );
    assert_eq!(CliVersion::extract_from_output("no version here"), None);
    assert_eq!(CliVersion::extract_from_output(""), None);
}

#[test]
fn versions_below_the_minimum_are_refused() -> Checked {
    let error = require_minimum_version(CliVersion::parse("0.47.99").required()?)
        .refused_because("an older version must be refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::SbxVersionBelowMinimum));
    Ok(())
}

#[test]
fn the_minimum_version_and_later_are_accepted() -> Checked {
    for observed in ["0.48.0", "0.48.1", "0.49.0", "1.0.0"] {
        assert!(
            require_minimum_version(CliVersion::parse(observed).required()?).is_ok(),
            "{observed} must be accepted"
        );
    }
    Ok(())
}

#[test]
fn only_three_numeric_parts_make_a_cli_version() {
    for value in ["1", "1.2", "1.2.3.4", "x.2.3", "1.x.3", "1.2.x", ""] {
        assert_eq!(CliVersion::parse(value), None, "{value}");
    }
    assert_eq!(
        CliVersion::parse(" 0.37.1\n"),
        Some(CliVersion {
            major: 0,
            minor: 37,
            patch: 1
        })
    );
}

#[test]
fn the_documented_minimum_is_the_one_this_build_requires() -> Checked {
    // 最低versionはこのconstantだけで決める。上げたのに要件の文書を合わせ忘れると、
    // 利用者は通らないversionを入れることになる。
    let stated = format!("Docker Sandboxes CLI {MINIMUM_CLI_VERSION}");
    for document in [
        "README.md",
        "docs/README.ja.md",
        "website/src/content/docs/getting-started/requirements.md",
        "website/src/content/docs/index.mdx",
    ] {
        let text = std::fs::read_to_string(format!("{}/{document}", env!("CARGO_MANIFEST_DIR")))
            .required_because(document)?;
        assert!(text.contains(&stated), "{document} must state {stated}");
    }
    Ok(())
}
