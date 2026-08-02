use crate::design::Fact;
use crate::diagnostics::ErrorId;
use crate::project::CanonicalProjectId;
use std::fs;
use std::path::Path;

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::paths::lexically_standardize;
use crate::testing::fs::temp_dir;

fn project_id(value: &str) -> Checked<CanonicalProjectId> {
    Ok(crate::project::ProjectId::parse(value)
        .required_because("valid project id")?
        .canonical())
}

#[test]
fn a_parent_directory_must_be_an_absolute_path_that_exists() -> Checked {
    let dir = temp_dir()?;
    ProjectParent::at(dir.path()).required_because("an existing directory can hold a project")?;

    for declared in [
        Path::new("relative/path").to_path_buf(),
        // sbxmは親directoryを作らない。存在しないpathは受け付けない。
        dir.path().join("not-created-yet"),
    ] {
        let error =
            ProjectParent::at(&declared).refused_because("{declared:?} cannot hold a project")?;
        assert_eq!(
            error.first_id(),
            Some(ErrorId::WorkingDirectoryUnusable),
            "{declared:?} produced the wrong error"
        );
    }
    Ok(())
}

#[test]
fn a_parent_directory_rejects_an_existing_regular_file() -> Checked {
    let dir = temp_dir()?;
    let file = dir.path().join("not-a-directory");
    fs::write(&file, b"x").required_because("write file")?;
    let error = ProjectParent::at(&file).refused_because("files cannot hold a project")?;
    assert_eq!(error.first_id(), Some(ErrorId::WorkingDirectoryUnusable));
    Ok(())
}

#[test]
fn project_paths_follow_the_documented_layout() -> Checked {
    let dir = temp_dir()?;
    let parent = ProjectParent::at(dir.path()).required_because("valid parent directory")?;
    let paths = ProjectPaths::derive(&parent, &project_id("Example-Org/Example-Repo")?);

    // owner名のdirectoryは作らない。project rootは親directoryの直下に並ぶ。
    assert_eq!(paths.root(), dir.path().join("example-repo.project"));
    let root = paths.root();
    assert_eq!(paths.host_clone(), root.join("example-repo"));
    assert_eq!(paths.metadata_file(), root.join(".sbxm/project.yaml"));
    assert_eq!(paths.lock_file(), root.join(".sbxm/project.lock"));
    assert_eq!(paths.dockerfile(), root.join(".sbxm/Dockerfile"));
    assert_eq!(paths.cache_dir(), root.join(".sbxm/.cache"));
    assert_eq!(
        paths.template_archive("0123456789ab"),
        root.join(".sbxm/.cache/template-0123456789ab.tar")
    );
    assert_eq!(
        paths.template_archive_temp("0123456789ab"),
        root.join(".sbxm/.cache/template-0123456789ab.tar.tmp")
    );
    Ok(())
}

#[test]
fn project_paths_are_lowercase_so_one_project_cannot_take_two_directories() -> Checked {
    let dir = temp_dir()?;
    let parent = ProjectParent::at(dir.path()).required_because("valid parent directory")?;
    assert_eq!(
        ProjectPaths::derive(&parent, &project_id("Example-Org/Example-Repo")?),
        ProjectPaths::derive(&parent, &project_id("example-org/example-repo")?)
    );
    Ok(())
}

#[test]
fn the_same_repository_name_under_two_owners_wants_the_same_directory() -> Checked {
    // owner名を足して衝突を避けないため、この2件は同じpathを要求する。衝突の扱いは
    // 登録側が決める。
    let dir = temp_dir()?;
    let parent = ProjectParent::at(dir.path()).required_because("valid parent directory")?;
    assert_eq!(
        ProjectPaths::derive(&parent, &project_id("example-org/alpha")?).root(),
        ProjectPaths::derive(&parent, &project_id("other-org/alpha")?).root()
    );
    Ok(())
}

#[test]
fn the_parent_of_a_new_project_is_the_directory_the_command_ran_in() -> Checked {
    // 置き場所はsbxmが選ばない。commandを実行したcurrent directoryをそのまま受け取る。
    let running_in =
        std::env::current_dir().required_because("the test process has a current directory")?;
    let parent =
        ProjectParent::current().required_because("the current directory can hold a project")?;
    assert_eq!(parent.as_path(), lexically_standardize(&running_in));
    Ok(())
}

#[test]
fn a_working_directory_that_cannot_be_read_is_reported_without_a_path() -> Checked {
    // 読めなかったのはcurrent directory自身である。指させるpathが無いため、
    // 診断はOSが書いた原因だけを持つ。
    let refused = std::io::Error::from_raw_os_error(2);
    let error = working_directory_unusable(refused);

    assert_eq!(error.first_id(), Some(ErrorId::WorkingDirectoryUnusable));
    let facts = &error
        .diagnostics()
        .first()
        .required_because("one diagnostic")?
        .facts;
    let quoted: Vec<&str> = facts
        .iter()
        .map(|fact| match fact {
            Fact::OneLine { value, .. } => value.as_str(),
            _ => "",
        })
        .collect();
    assert_eq!(
        quoted,
        vec![std::io::Error::from_raw_os_error(2).to_string().as_str()],
        "the only fact is what the operating system said"
    );
    Ok(())
}
