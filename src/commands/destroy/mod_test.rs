use super::*;
use crate::commands::Command;
use crate::testing::cli::{command, non_tty, parse_argv, tty};

#[test]
fn forced_destroy_always_requires_a_fully_specified_project() {
    for flag in ["--force", "-f"] {
        let error = parse_argv(&["destroy", flag], tty())
            .expect_err("force mode never prompts for a target");
        assert_eq!(error.first_id(), Some(ErrorId::ProjectArgumentRequired));

        assert_eq!(
            command(&["destroy", flag, "owner/repo"], non_tty()),
            Command::Destroy(Args {
                project: Some(ProjectId::parse("owner/repo").unwrap()),
                force: true
            })
        );
    }
}
