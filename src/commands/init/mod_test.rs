use super::*;
use crate::commands::Command;
use crate::i18n::{Catalog, Locale};
use crate::testing::cli::{command, non_tty, parse_argv, tty};

#[test]
fn init_accepts_either_no_options_or_all_three() {
    assert_eq!(
        command(&["init"], non_tty()),
        Command::Init(Mode::Interactive)
    );
    assert_eq!(
        command(
            &[
                "init",
                "--base-path",
                "/Users/example/Projects",
                "--git-user-name",
                "Example User",
                "--git-user-email",
                "user@example.com"
            ],
            non_tty()
        ),
        Command::Init(Mode::Options {
            base_path: "/Users/example/Projects".into(),
            git_user_name: "Example User".into(),
            git_user_email: "user@example.com".into(),
        })
    );
}

#[test]
fn a_partially_specified_init_is_refused_before_anything_is_read() {
    let error = parse_argv(&["init", "--base-path", "/tmp/projects"], tty())
        .expect_err("a partial option set is refused");
    assert_eq!(error.first_id(), Some(ErrorId::InitIncompleteOptions));
    let rendered = Catalog::new(Locale::En)
        .format(&error.diagnostics()[0].description)
        .expect("the diagnostic formats");
    assert!(rendered.contains("--git-user-name"), "{rendered}");
    assert!(rendered.contains("--git-user-email"), "{rendered}");
}

#[test]
fn the_init_mode_is_decided_without_looking_at_lang() {
    assert_eq!(
        command(&["--lang", "ja", "init"], non_tty()),
        Command::Init(Mode::Interactive)
    );
    assert_eq!(
        command(
            &[
                "init",
                "--lang",
                "en",
                "--base-path",
                "/tmp/p",
                "--git-user-name",
                "n",
                "--git-user-email",
                "e"
            ],
            non_tty()
        ),
        Command::Init(Mode::Options {
            base_path: "/tmp/p".into(),
            git_user_name: "n".into(),
            git_user_email: "e".into(),
        })
    );
}
