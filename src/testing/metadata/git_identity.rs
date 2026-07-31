use crate::metadata::GitIdentity;

/// testが使う、利用者が選んだことになっているGit identity。
pub fn git_identity() -> GitIdentity {
    GitIdentity {
        user_name: "Example User".to_string(),
        user_email: "user@example.com".to_string(),
    }
}
