//! registryの記録と、案件directoryに置かれたmetadataが食い違う状態からの再実行。
//!
//! どちらも利用者が読み書きできるYAMLである。両者が別々の案件や別々のtransportを
//! 名乗る状態は、上書きして辻褄を合わせるのではなく、観測した食い違いを示して止める。

use crate::testing::outcome::{Checked, Refused, Required};

use super::*;
use crate::design::text::CommandLine;
use crate::testing::add_request::request;
use crate::testing::project::{https_repository, ssh_repository};

/// 親directoryとglobal state directoryを持つtest環境。
struct Setup {
    dir: tempfile::TempDir,
    _home: tempfile::TempDir,
    location: ConfigLocation,
    parent: ProjectParent,
}

fn setup() -> Checked<Setup> {
    let dir = tempfile::tempdir().required_because("temporary parent directory")?;
    let home = tempfile::tempdir().required_because("temporary home")?;
    Ok(Setup {
        location: ConfigLocation::from_home(home.path().to_path_buf()),
        parent: ProjectParent::at(dir.path()).required_because("valid parent directory")?,
        dir,
        _home: home,
    })
}

fn identity() -> GitIdentity {
    crate::testing::metadata::git_identity()
}

/// 保存済みmetadataのrepositoryだけを差し替える。
fn store_repository(
    paths: &ProjectPaths,
    metadata: &ProjectMetadata,
    repository: RepositoryIdentity,
) -> Checked {
    let mut tampered = metadata.clone();
    tampered.repository = repository;
    metadata::update(paths, &tampered).required_because("store the edited metadata")?;
    Ok(())
}

#[test]
fn a_project_root_whose_metadata_names_another_project_is_a_path_collision() -> Checked {
    let setup = setup()?;
    let asked = request("example-org/example-repo", None, None)?;
    let registration = register(&setup.location, &setup.parent, &asked, &identity())?;
    let paths = registration.paths.clone();
    let metadata = registration.metadata.clone();
    drop(registration);

    // registryが指すrootに、別の案件のmetadataが置かれている状態。
    store_repository(&paths, &metadata, ssh_repository("other-org/other-repo")?)?;
    let before = fs::read_to_string(paths.metadata_file()).required()?;

    let error = register(&setup.location, &setup.parent, &asked, &identity()).refused()?;
    assert_eq!(error.first_id(), Some(ErrorId::ProjectPathCollision));

    // どちらの案件がそのrootを占めているのかを名指しする。
    let description = &error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?
        .description;
    assert!(
        description
            .args
            .contains(&("observed", "other-org/other-repo".to_string()))
            && description
                .args
                .contains(&("requested", "example-org/example-repo".to_string())),
        "the refusal names the occupant and the request: {:?}",
        description.args
    );
    assert_eq!(
        fs::read_to_string(paths.metadata_file()).required()?,
        before,
        "a refused run never rewrites the metadata it read"
    );
    Ok(())
}

#[test]
fn a_stored_clone_url_that_disagrees_with_the_registry_stops_the_run() -> Checked {
    let setup = setup()?;
    let asked = request("Example-Org/Example-Repo", None, None)?;
    let registration = register(&setup.location, &setup.parent, &asked, &identity())?;
    let paths = registration.paths.clone();
    let metadata = registration.metadata.clone();
    drop(registration);

    // registry entryはSSHのまま、metadataだけがHTTPSを名乗る状態。
    store_repository(
        &paths,
        &metadata,
        https_repository("Example-Org/Example-Repo")?,
    )?;

    let error = register(&setup.location, &setup.parent, &asked, &identity()).refused()?;
    assert_eq!(error.first_id(), Some(ErrorId::TargetConfigurationMismatch));

    let diagnostic = error
        .diagnostics()
        .first()
        .required_because("the refusal carries a diagnostic")?;
    assert!(
        diagnostic.description.args.contains(&(
            "requested",
            "git@github.com:Example-Org/Example-Repo.git".to_string()
        )) && diagnostic.description.args.contains(&(
            "stored",
            "https://github.com/Example-Org/Example-Repo.git".to_string()
        )),
        "both spellings are named rather than merged: {:?}",
        diagnostic.description.args
    );

    // 再実行の案内は保存済みの綴りを示す。この実行のURLで登録を書き換えさせない。
    let remediation = diagnostic
        .remediation
        .as_ref()
        .required_because("the refusal says what to do")?;
    assert_eq!(
        remediation
            .commands
            .iter()
            .map(CommandLine::as_str)
            .collect::<Vec<_>>(),
        vec!["sbxm add https://github.com/Example-Org/Example-Repo.git"]
    );
    Ok(())
}

#[test]
fn a_registry_that_could_not_be_written_records_nothing_and_creates_no_root() -> Checked {
    let setup = setup()?;
    register(
        &setup.location,
        &setup.parent,
        &request("example-org/alpha", None, None)?,
        &identity(),
    )?;

    // 中断した実行が残した一時fileは、次の実行が黙って再利用してよいものではない。
    let leftover = setup.location.dir().join(".registry.yaml.tmp");
    fs::write(&leftover, b"interrupted\n").required()?;

    let error = register(
        &setup.location,
        &setup.parent,
        &request("other-org/beta", None, None)?,
        &identity(),
    )
    .refused_because("the registry cannot be written while the leftover is there")?;
    assert_eq!(error.first_id(), Some(ErrorId::TempFileLeftBehind));

    // 記録できなかった登録の成果物は、1つも作らない。
    assert!(
        !setup.dir.path().join("beta.project").exists(),
        "no project root is created for a registration that was never recorded"
    );
    let registry = crate::registry::load(&setup.location)
        .required_because("the registry is still readable")?;
    assert_eq!(
        registry.entries().len(),
        1,
        "the registry holds only what was written"
    );
    assert_eq!(
        fs::read_to_string(&leftover).required()?,
        "interrupted\n",
        "the leftover is left for the reader to look at"
    );
    Ok(())
}
