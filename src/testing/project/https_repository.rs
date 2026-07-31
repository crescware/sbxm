use crate::testing::outcome::{Checked, Required};

use crate::repository::RepositoryIdentity;

/// `<owner>/<repository>`からHTTPS clone `URLのidentityを作る`。
pub fn https_repository(value: &str) -> Checked<RepositoryIdentity> {
    RepositoryIdentity::parse_clone_url(&format!("https://github.com/{value}.git"))
        .required_because("valid clone URL")
}
