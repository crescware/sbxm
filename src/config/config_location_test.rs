use crate::testing::outcome::{Checked, Refused, Required};

use super::*;

impl ConfigLocation {
    /// 指定したhome directoryから組み立てる。
    pub fn from_home(home: PathBuf) -> ConfigLocation {
        ConfigLocation { home }
    }
}

#[test]
fn a_known_home_directory_anchors_every_path() -> Checked {
    let location = ConfigLocation::from_home_directory(Some(PathBuf::from("/Users/example")))
        .required_because("a known home directory builds the location")?;
    assert_eq!(
        location.config_file(),
        PathBuf::from("/Users/example/.sbxm/config.yaml")
    );
    Ok(())
}

#[test]
fn an_unknown_home_directory_is_refused_rather_than_guessed() -> Checked {
    let error = ConfigLocation::from_home_directory(None)
        .refused_because("without a home directory no path can be built")?;

    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?;
    assert_eq!(diagnostic.id, ErrorId::ConfigUnreadable);
    // pathを1つも組み立てられない以上、pathは添えない。示せるのは読めなかった理由だけである。
    assert_eq!(diagnostic.facts.len(), 1);
    let reason = match &diagnostic.facts[0] {
        Fact::Translated { value, .. } => Some(value.id),
        _ => None,
    }
    .required_because("the reason is a message of sbxm's own, not external text")?;
    assert_eq!(reason, "cause-home-directory-unknown");
    Ok(())
}
