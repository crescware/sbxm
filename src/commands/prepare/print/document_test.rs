//! `prepare`のworktree表。

use crate::boundary::host::protocol::SandboxState;
use crate::i18n::Locale;
use crate::metadata::CreationMode;

use crate::testing::outcome::{Checked, Required};
use crate::testing::plain;
use crate::testing::value::COMMIT;

use crate::support::provisioning::WorktreeRow;

use super::*;

/// 1本のworktreeを持つ実行結果。
fn output() -> PrepareOutput {
    PrepareOutput {
        project: "Example-Org/Example-Repo".to_string(),
        sandbox: "sbxm-example-org-example-repo-99a40327a69b".to_string(),
        mode: CreationMode::Attached,
        start_ref: "main".to_string(),
        sandbox_state: SandboxState::Running,
        worktrees: vec![WorktreeRow {
            path: "example-repo.tree-0".to_string(),
            created_from: "refs/remotes/origin/main".to_string(),
            head: COMMIT.to_string(),
            mode: CreationMode::Attached,
        }],
        files: Vec::new(),
        already_built: false,
        warnings: Vec::new(),
    }
}

/// worktreeの行。
fn row(text: &str) -> Checked<String> {
    Ok(text
        .lines()
        .find(|line| line.contains("example-repo.tree-0"))
        .required_because("the worktree is listed")?
        .to_string())
}

#[test]
fn a_worktree_row_names_its_observed_head() -> Checked {
    let known = row(&plain(&document(&output(), Locale::En), Locale::En)?)?;
    assert!(known.contains(COMMIT), "{known}");
    Ok(())
}

#[test]
fn the_completed_run_says_what_it_built_instead_of_naming_a_missing_message() -> Checked {
    // messageを引けなかった行はrendererが内部異常の文字列へ置き換える。それが成功marker
    // の後ろに並ぶと、構築は済んでいるのに壊れた実行に見える。
    for locale in Locale::ALL {
        let drawn = plain(&document(&output(), locale), locale)?;
        let summary = drawn.lines().next().required_because("the summary")?;

        assert!(
            !summary.contains("message-format-failed"),
            "{locale:?}: {summary}"
        );
        assert!(
            summary.contains("Example-Org/Example-Repo")
                && summary.contains("sbxm-example-org-example-repo-99a40327a69b"),
            "the summary names the project and the sandbox it built, {locale:?}: {summary}"
        );
    }
    Ok(())
}
