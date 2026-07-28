use super::*;

#[test]
fn the_porcelain_listing_is_read_field_by_field() {
    let output = "worktree /home/agent/work/repo\0bare\0\0worktree /home/agent/work/repo/repo.tree-0\0HEAD abc\0branch refs/heads/main\0\0worktree /home/agent/work/repo/repo.tree-1\0HEAD abc\0detached\0\0";
    let entries = parse_list(output).expect("the listing parses");
    assert_eq!(
        entries,
        vec![
            Entry {
                path: "/home/agent/work/repo".to_string(),
                bare: true,
                detached: false,
            },
            Entry {
                path: "/home/agent/work/repo/repo.tree-0".to_string(),
                bare: false,
                detached: false,
            },
            Entry {
                path: "/home/agent/work/repo/repo.tree-1".to_string(),
                bare: false,
                detached: true,
            },
        ]
    );
    assert!(parse_list("").expect("an empty listing").is_empty());
    assert!(parse_list("detached\0\0").is_err());
}

#[test]
fn only_paths_under_the_bare_root_are_this_projects_worktrees() {
    let root = "/home/agent/work/repo";
    let managed = Entry {
        path: format!("{root}/repo.tree-0"),
        bare: false,
        detached: false,
    };
    assert_eq!(managed.relative_to(root).as_deref(), Some("repo.tree-0"));

    let bare = Entry {
        path: root.to_string(),
        bare: true,
        detached: false,
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
    };
    assert_eq!(escaping.relative_to(root), None);

    // 名前の前方一致だけでbare root配下とみなさない。
    let sibling = Entry {
        path: format!("{root}-other/tree"),
        bare: false,
        detached: false,
    };
    assert_eq!(sibling.relative_to(root), None);
}
