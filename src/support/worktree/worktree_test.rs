use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::diagnostics::ErrorId;
use crate::testing::repository::layout;
use crate::testing::sandbox::InnerCommandSandbox;

#[test]
fn inspection_progress_names_each_worktree_without_exposing_control_characters() -> Checked {
    use crate::testing::host::FakeSbx;
    use crate::testing::recorded_output::RecordedOutput;

    let host = FakeSbx::listing(r#"{"sandboxes":[]}"#);
    let mut progress = RecordedOutput::new();
    let paths = ["/work/repo.tree-0", "/work/改行\nと\u{1b}[31m.tree-1"];
    for (index, path) in paths.iter().enumerate() {
        let mut inspection = Inspection::new(
            &host,
            "sandbox",
            path,
            index + 1,
            paths.len(),
            &mut progress,
        );
        inspection.changes()?;
        inspection.head()?;
    }
    assert_eq!(progress.steps.len(), 4);
    for (index, path) in paths.iter().enumerate() {
        for (offset, id) in [
            "progress-inspect-worktree-changes",
            "progress-inspect-worktree-head",
        ]
        .iter()
        .enumerate()
        {
            let message = &progress.steps[index * 2 + offset];
            assert_eq!(message.id, *id);
            assert!(message.args.contains(&("current", (index + 1).to_string())));
            assert!(message.args.contains(&("total", "2".to_string())));
            assert!(message.args.contains(&("path", format!("{path:?}"))));
            assert!(
                message
                    .args
                    .iter()
                    .all(|(_, value)| !value.contains(['\n', '\u{1b}']))
            );
        }
        assert!(
            host.calls()
                .iter()
                .any(|args| args.iter().any(|arg| arg == path)),
            "the observed path is not escaped"
        );
    }
    Ok(())
}

#[test]
fn the_porcelain_listing_is_read_field_by_field() -> Checked {
    let output = "worktree /home/agent/work/repo\0bare\0\0worktree /home/agent/work/repo/repo.tree-0\0HEAD abc\0branch refs/heads/main\0\0worktree /home/agent/work/repo/repo.tree-1\0HEAD abc\0detached\0\0worktree /home/agent/work/repo/repo.tree-2\0HEAD abc\0branch refs/heads/topic\0prunable gitdir file points to non-existent location\0\0";
    let entries = parse_list(output).required_because("the listing parses")?;
    assert_eq!(
        entries,
        vec![
            Entry {
                path: "/home/agent/work/repo".to_string(),
                bare: true,
                detached: false,
                branch: None,
                prunable: false,
            },
            Entry {
                path: "/home/agent/work/repo/repo.tree-0".to_string(),
                bare: false,
                detached: false,
                branch: Some("refs/heads/main".to_string()),
                prunable: false,
            },
            Entry {
                path: "/home/agent/work/repo/repo.tree-1".to_string(),
                bare: false,
                detached: true,
                branch: None,
                prunable: false,
            },
            Entry {
                path: "/home/agent/work/repo/repo.tree-2".to_string(),
                bare: false,
                detached: false,
                branch: Some("refs/heads/topic".to_string()),
                prunable: true,
            },
        ]
    );
    assert!(parse_list("").is_err(), "an empty listing is unobservable");
    assert!(parse_list("detached\0\0").is_err());
    Ok(())
}

#[test]
fn an_incomplete_record_is_rejected() {
    assert!(
        parse_list("worktree /home/agent/work/repo/repo.tree-0\0HEAD abc").is_err(),
        "a record without its final separator is unobservable"
    );
    assert!(
        parse_list(
            "worktree /home/agent/work/repo\0worktree /home/agent/work/repo/repo.tree-0\0\0"
        )
        .is_err(),
        "a new record cannot begin before the previous record closes"
    );
}

#[test]
fn unknown_and_malformed_records_are_rejected() {
    assert!(parse_list("\0\0").is_err());
    assert!(parse_list("unexpected value\0\0").is_err());
    assert!(parse_list("worktree /home/agent/work/repo\0\0\0\0").is_err());
    assert!(parse_list("worktree \0\0").is_err());
    assert!(parse_list("worktree /home/agent/work/repo\0unknown\0\0").is_err());
    assert!(parse_list("worktree /home/agent/work/repo\0\0").is_err());
    assert!(parse_list("worktree /home/agent/work/repo\0bare value\0\0").is_err());
    assert!(parse_list("worktree /home/agent/work/repo\0bare\0HEAD abc\0\0").is_err());
    assert!(parse_list("worktree /home/agent/work/repo\0detached value\0\0").is_err());
    assert!(parse_list("worktree /home/agent/work/repo\0branch main\0locked\0locked\0\0").is_err());
    assert!(
        parse_list("worktree /home/agent/work/repo\0branch main\0prunable\0prunable\0\0").is_err()
    );
    assert!(parse_list("worktree /home/agent/work/repo\0HEAD abc def\0\0").is_err());
    assert!(
        parse_list("worktree /home/agent/work/repo\0branch refs/heads/main extra\0\0").is_err()
    );
    assert!(
        parse_list(
            "worktree /home/agent/work/repo\0branch refs/heads/main\0branch refs/heads/dev\0\0"
        )
        .is_err()
    );
}

#[test]
fn a_listing_the_host_could_not_run_is_not_read_as_no_worktrees() -> Checked {
    // 一覧が空であることと、一覧を読めなかったことは違う。読めなかった側を空と
    // 見なすと、案件の成果物が1つも無いという判断がそこから続く。
    let git_dir = layout()?.bare_git_dir();
    let host = InnerCommandSandbox::new().timing_out(&format!(
        "git --git-dir {git_dir} worktree list --porcelain -z"
    ));

    let error = list(&host, "sbxm-example", &layout()?)
        .refused_because("a listing that did not run is not an empty listing")?;
    assert_eq!(error.first_id(), Some(ErrorId::ExternalCommandTimeout));
    Ok(())
}

#[test]
fn only_paths_under_the_bare_root_are_this_projects_worktrees() {
    let root = "/home/agent/work/repo";
    let managed = Entry {
        path: format!("{root}/repo.tree-0"),
        bare: false,
        detached: false,
        branch: None,
        prunable: false,
    };
    assert_eq!(managed.relative_to(root).as_deref(), Some("repo.tree-0"));

    let bare = Entry {
        path: root.to_string(),
        bare: true,
        detached: false,
        branch: None,
        prunable: false,
    };
    assert_eq!(
        bare.relative_to(root),
        None,
        "a bare entry is not a worktree"
    );

    // `..`を含むpathは、standardizeした結果で判定する。
    let escaping = Entry {
        path: format!("{root}/../elsewhere"),
        bare: false,
        detached: false,
        branch: None,
        prunable: false,
    };
    assert_eq!(escaping.relative_to(root), None);

    // 名前の前方一致だけでbare root配下とみなさない。
    let sibling = Entry {
        path: format!("{root}-other/tree"),
        bare: false,
        detached: false,
        branch: None,
        prunable: false,
    };
    assert_eq!(sibling.relative_to(root), None);
}

#[test]
fn an_empty_record_between_records_is_rejected() {
    assert!(
        parse_list("worktree /home/agent/work/repo\0bare\0\0\0\0").is_err(),
        "a record with no field is not a worktree"
    );
}
