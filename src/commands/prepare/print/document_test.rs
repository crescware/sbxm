//! `prepare`のworktree表が、観測できなかった値をどう置くか。

use crate::compatibility::SandboxState;
use crate::i18n::Locale;
use crate::metadata::CreationMode;

use crate::testing::outcome::{Checked, Required};
use crate::testing::plain;
use crate::testing::value::COMMIT;

use crate::commands::prepare::WorktreeRow;

use super::*;

/// 1本のworktreeを持つ実行結果。HEADの有無だけを差し替える。
fn output(head: Option<&str>) -> PrepareOutput {
    PrepareOutput {
        project: "Example-Org/Example-Repo".to_string(),
        sandbox: "sbxm-example-org-example-repo-99a40327a69b".to_string(),
        mode: CreationMode::Attached,
        start_ref: "main".to_string(),
        sandbox_state: SandboxState::Running,
        worktrees: vec![WorktreeRow {
            path: "example-repo.tree-0".to_string(),
            created_from: "refs/remotes/origin/main".to_string(),
            head: head.map(str::to_string),
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
fn a_head_that_could_not_be_read_is_shown_as_unknown_rather_than_left_blank() -> Checked {
    // 停止中のSandboxではHEADを読めない。空欄にすると、そのworktreeにcommitが無いのか
    // 読めなかったのかを区別できなくなる。列は残し、値が無いことを示す。
    let known = row(&plain(
        &document(&output(Some(COMMIT)), Locale::En),
        Locale::En,
    )?)?;
    assert!(known.contains(COMMIT), "{known}");

    let unknown = row(&plain(&document(&output(None), Locale::En), Locale::En)?)?;
    assert!(!unknown.contains(COMMIT), "{unknown}");
    let cells: Vec<&str> = unknown
        .split("  ")
        .map(str::trim)
        .filter(|cell| !cell.is_empty())
        .collect();
    assert_eq!(
        cells,
        vec![
            "example-repo.tree-0",
            "refs/remotes/origin/main",
            "-",
            "attached"
        ],
        "the row keeps its columns and marks the head it could not read"
    );
    Ok(())
}

#[test]
fn the_completed_run_says_what_it_built_instead_of_naming_a_missing_message() -> Checked {
    // messageを引けなかった行はrendererが内部異常の文字列へ置き換える。それが成功marker
    // の後ろに並ぶと、構築は済んでいるのに壊れた実行に見える。
    for locale in Locale::ALL {
        let drawn = plain(&document(&output(Some(COMMIT)), locale), locale)?;
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
