use crate::diagnostics::ErrorId;

use crate::testing::outcome::{Checked, Refused};

use crate::commands::{Command, apply::Args};
use crate::testing::cli::{command, parse_argv, tty};

#[test]
fn apply_requires_an_explicit_scope() -> Checked {
    let error = parse_argv(&["apply", "owner/repo"], tty())
        .refused_because("apply without a scope is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ApplyScopeRequired));

    assert!(matches!(
        command(&["apply", "owner/repo", "--files"], tty())?,
        Command::Apply(Args {
            files: true,
            worktrees: None,
            ..
        })
    ));
    assert!(matches!(
        command(&["apply", "owner/repo", "--worktrees", "3"], tty())?,
        Command::Apply(Args {
            files: false,
            worktrees: Some(3),
            ..
        })
    ));
    assert!(matches!(
        command(
            &["apply", "owner/repo", "--files", "--worktrees", "3"],
            tty()
        )?,
        Command::Apply(Args {
            files: true,
            worktrees: Some(3),
            ..
        })
    ));
    Ok(())
}

/// `-t`は`--worktrees`の別名であり、同じ本数として解釈される。
#[test]
fn the_short_form_requests_the_same_worktree_count() -> Checked {
    assert!(matches!(
        command(&["apply", "owner/repo", "-t", "3"], tty())?,
        Command::Apply(Args {
            files: false,
            worktrees: Some(3),
            ..
        })
    ));
    Ok(())
}

#[test]
fn force_is_accepted_only_together_with_files() -> Checked {
    assert!(matches!(
        command(&["apply", "owner/repo", "--files", "--force"], tty())?,
        Command::Apply(Args {
            files: true,
            force: true,
            ..
        })
    ));
    assert!(matches!(
        command(&["apply", "owner/repo", "--files"], tty())?,
        Command::Apply(Args { force: false, .. })
    ));

    // worktreeだけを適用する実行には、置き換えを許す宣言fileが無い。
    let error = parse_argv(
        &["apply", "owner/repo", "--worktrees", "3", "--force"],
        tty(),
    )
    .refused_because("--force without --files is refused")?;
    assert_eq!(error.first_id(), Some(ErrorId::ApplyForceWithoutFiles));
    Ok(())
}

#[test]
fn all_takes_the_place_of_a_project_and_needs_no_terminal() -> Checked {
    // 全案件が対象であり、選ぶ案件が無い。非対話端末でもそのまま受け付ける。
    for interactivity in [tty(), crate::testing::cli::non_tty()] {
        assert!(matches!(
            command(&["apply", "--files", "--all"], interactivity)?,
            Command::Apply(Args {
                project: None,
                all: true,
                files: true,
                ..
            })
        ));
    }
    assert!(matches!(
        command(&["apply", "--files", "--all", "--force"], tty())?,
        Command::Apply(Args {
            all: true,
            force: true,
            ..
        })
    ));
    Ok(())
}

#[test]
fn all_cannot_be_combined_with_a_project_or_a_worktree_count() -> Checked {
    for arguments in [
        vec!["apply", "owner/repo", "--files", "--all"],
        vec!["apply", "--files", "--all", "--worktrees", "3"],
    ] {
        let error =
            parse_argv(&arguments, tty()).refused_because(&format!("{arguments:?} is refused"))?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::ConflictingArguments),
            "{arguments:?}"
        );
    }
    // 何を配置するかは`--all`だけでは決まらない。
    let error = parse_argv(&["apply", "--all"], tty())
        .refused_because("--all alone names nothing to apply")?;
    assert_eq!(error.first_id(), Some(ErrorId::ApplyScopeRequired));
    Ok(())
}
