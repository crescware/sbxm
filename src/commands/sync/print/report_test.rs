use std::path::PathBuf;

use crate::commands::sync::SentChange;
use crate::design::RenderingPolicy;
use crate::i18n::Locale;
use crate::support::host_sync::{ReflectResult, Reflected};

use crate::testing::outcome::{Checked, Required};

use super::*;

fn printed(reflected: Option<Vec<Reflected>>) -> Checked<(ExitCode, String)> {
    printed_with(reflected, Vec::new())
}

fn printed_with(
    reflected: Option<Vec<Reflected>>,
    sent: Vec<SentChange>,
) -> Checked<(ExitCode, String)> {
    let output = SyncOutput {
        project: "local/app".to_string(),
        repository: PathBuf::from("/Users/example/code/app/.git"),
        namespace: "sbxm-local-app-0123456789ab".to_string(),
        reflected,
        sent,
    };
    let mut stdout: Vec<u8> = Vec::new();
    let code = {
        let mut ui = Ui::capture(
            Locale::En,
            RenderingPolicy::plain(),
            &mut stdout,
            std::io::sink(),
        );
        report(&mut ui, &output)
    };
    Ok((code, String::from_utf8(stdout).required_because("UTF-8")?))
}

fn reflected(reference: &str, result: ReflectResult) -> Reflected {
    Reflected {
        reference: reference.to_string(),
        result,
    }
}

#[test]
fn a_ref_git_left_as_it_was_ends_the_sync_with_a_failure_after_the_result() -> Checked {
    // `git push`と同じく、断られたrefがあれば`1`で終わる。結果はそれでも示す。
    for result in [
        ReflectResult::Diverged,
        ReflectResult::CheckedOut,
        ReflectResult::Exists,
        ReflectResult::Refused {
            reason: "pre-receive hook declined".to_string(),
        },
    ] {
        let (code, stdout) = printed(Some(vec![
            reflected("refs/heads/topic", ReflectResult::Updated),
            reflected("refs/heads/main", result.clone()),
        ]))?;
        assert_eq!(code, ExitCode::Failure, "{result:?}");
        assert!(stdout.contains("refs/heads/main"), "{stdout}");
    }
    Ok(())
}

#[test]
fn a_sandbox_branch_that_is_only_behind_does_not_fail_the_sync() -> Checked {
    // 遅れているだけのbranchは、hostが既にそのcommitを持つ。失うものは無い。
    for reflected_refs in [
        None,
        Some(Vec::new()),
        Some(vec![
            reflected("refs/heads/main", ReflectResult::Behind),
            reflected("refs/heads/topic", ReflectResult::Created),
            reflected("refs/tags/v1", ReflectResult::Created),
            reflected("refs/heads/side", ReflectResult::Updated),
        ]),
    ] {
        let (code, _) = printed(reflected_refs.clone())?;
        assert_eq!(code, ExitCode::Success, "{reflected_refs:?}");
    }
    Ok(())
}

#[test]
fn a_tag_the_sandbox_kept_ends_the_sync_with_a_failure_too() -> Checked {
    // Sandboxの中の`git fetch`も、既存のtagを上書きできなければ失敗で終わる。
    let (code, stdout) = printed_with(
        Some(Vec::new()),
        vec![SentChange::Refused {
            reference: "refs/tags/v1".to_string(),
            reason: "already exists".to_string(),
        }],
    )?;
    assert_eq!(code, ExitCode::Failure);
    assert!(stdout.contains("refs/tags/v1"), "{stdout}");

    let (code, _) = printed_with(
        Some(Vec::new()),
        vec![
            SentChange::Created {
                reference: "refs/remotes/origin/topic".to_string(),
            },
            SentChange::Updated {
                reference: "refs/remotes/origin/main".to_string(),
            },
            SentChange::Removed {
                reference: "refs/remotes/origin/old".to_string(),
            },
        ],
    )?;
    assert_eq!(code, ExitCode::Success);
    Ok(())
}
