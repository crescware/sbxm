use crate::testing::outcome::{Checked, Required};

use crate::repository::RepositoryIdentity;

/// `<owner>/<repository>`からSSH clone `URLのidentityを作る`。
pub fn ssh_repository(value: &str) -> Checked<RepositoryIdentity> {
    RepositoryIdentity::parse_clone_url(&format!("git@github.com:{value}.git"))
        .required_because("valid clone URL")
}
